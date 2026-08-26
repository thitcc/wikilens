//! Persistent user settings — the growing config panel's store.
//!
//! Backing file: `settings.json` in the Tauri app-data dir, next to
//! `wikis.json`. Loaded once in `lib.rs`'s setup — *before* tray creation and
//! hotkey registration, both of which read the configured combos from here —
//! and managed as Tauri state. Mutations follow the `UserWikiStore`
//! discipline: persisted to disk before they become visible in memory.
//! Unknown JSON keys written by a future version are preserved across saves,
//! so an older binary never strips a newer one's settings.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

use crate::error::AppError;
use crate::hotkey;

/// Which configurable shortcut a command refers to. Serialized lowercase —
/// the wire form the frontend sends (`"summon"` / `"capture"`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyRole {
    Summon,
    Capture,
}

/// The resolved, always-valid pair of live shortcuts.
#[derive(Clone, Copy)]
pub struct Hotkeys {
    pub summon: Shortcut,
    pub capture: Shortcut,
}

/// Which model-source mode the player chose in the config panel — `"custom"`
/// / `"local"` on disk and on the wire
/// (vault/2026-08-24_replace-default-mode-with-local-ai.md). The serde form
/// (the `set_mode` command / `SettingsInfo`) and the `as_str` file form are
/// pinned against each other by `mode_serde_matches_the_stored_wire_strings`
/// and `set_mode_persists_and_reloads`, so the two encodings can't drift.
/// The removed `"default"` (Built In) survives on disk in old installs;
/// `resolve_mode` degrades it to unchosen with a removal notice.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Custom,
    Local,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Custom => "custom",
            Mode::Local => "local",
        }
    }

    fn parse(s: &str) -> Option<Mode> {
        match s {
            "custom" => Some(Mode::Custom),
            "local" => Some(Mode::Local),
            _ => None,
        }
    }
}

/// Where an anchored overlay docks. Kebab-case on disk and on the wire
/// (`"top-right"` …); the serde form and `as_str` are pinned against each
/// other like `Mode`'s.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PanelAnchor {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    Center,
}

impl PanelAnchor {
    fn as_str(self) -> &'static str {
        match self {
            PanelAnchor::TopRight => "top-right",
            PanelAnchor::TopLeft => "top-left",
            PanelAnchor::BottomRight => "bottom-right",
            PanelAnchor::BottomLeft => "bottom-left",
            PanelAnchor::Center => "center",
        }
    }

    fn parse(s: &str) -> Option<PanelAnchor> {
        match s {
            "top-right" => Some(PanelAnchor::TopRight),
            "top-left" => Some(PanelAnchor::TopLeft),
            "bottom-right" => Some(PanelAnchor::BottomRight),
            "bottom-left" => Some(PanelAnchor::BottomLeft),
            "center" => Some(PanelAnchor::Center),
            _ => None,
        }
    }
}

/// Whether the overlay follows its anchor or the player's dragged spot.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PositionMode {
    Anchored,
    Manual,
}

impl PositionMode {
    fn as_str(self) -> &'static str {
        match self {
            PositionMode::Anchored => "anchored",
            PositionMode::Manual => "manual",
        }
    }

    fn parse(s: &str) -> Option<PositionMode> {
        match s {
            "anchored" => Some(PositionMode::Anchored),
            "manual" => Some(PositionMode::Manual),
            _ => None,
        }
    }
}

/// Which vertical edge a dropped Manual spot pins. `Top` when a cap-height
/// window still fits below the drop (growth and menus keep opening
/// downward); `Bottom` when it doesn't — the window's bottom edge at drop
/// time becomes the invariant and everything opens upward, the
/// bottom-anchor behavior the drop visually resembles. The edge crosses IPC
/// (`PositionInfo.manualEdge`) so the frontend can flip the menus; the
/// coordinate never does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManualEdge {
    Top,
    Bottom,
}

impl ManualEdge {
    fn as_str(self) -> &'static str {
        match self {
            ManualEdge::Top => "top",
            ManualEdge::Bottom => "bottom",
        }
    }

    fn parse(s: &str) -> Option<ManualEdge> {
        match s {
            "top" => Some(ManualEdge::Top),
            "bottom" => Some(ManualEdge::Bottom),
            _ => None,
        }
    }
}

/// A dragged spot: the window's outer `x`, plus `y` as the pinned edge's
/// coordinate — the window TOP for `edge: Top`, the window BOTTOM for
/// `edge: Bottom`. Physical virtual-screen px (what `WindowEvent::Moved`
/// delivers).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ManualSpot {
    pub x: i32,
    pub y: i32,
    pub edge: ManualEdge,
}

/// The overlay placement choice. Both memories persist independently: `anchor`
/// survives Manual picks and drags, `manual` survives anchor picks — stepping
/// between them restores each. Coordinates never cross IPC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PanelPosition {
    pub mode: PositionMode,
    pub anchor: PanelAnchor,
    /// Last dragged spot; `None` until the panel is first dragged (or Manual
    /// is first picked, which snapshots the current position). Manual mode
    /// with no stored spot lays out from `anchor`.
    pub manual: Option<ManualSpot>,
    /// The padlock: `true` = the header never drags, in every mode.
    pub locked: bool,
}

impl Default for PanelPosition {
    fn default() -> Self {
        PanelPosition {
            mode: PositionMode::Anchored,
            anchor: PanelAnchor::TopRight,
            manual: None,
            locked: false,
        }
    }
}

/// The Local AI mode's stored configuration.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LocalAi {
    /// Normalized base URL (the command layer runs
    /// `target::normalize_local_base_url` before storing); `None` = use the
    /// baked `target::LOCAL_DEFAULT_BASE_URL`.
    pub base_url: Option<String>,
    /// The manual "reads images" toggle — local `/v1/models` payloads carry
    /// no capability metadata, so the player declares it. Default off.
    pub vision: bool,
}

/// Everything `persist` writes, mutated as one candidate (clone-mutate-
/// persist-commit). One write lock held across persist serializes every
/// mutation, so two concurrent mutators can never save each other's state
/// stale.
#[derive(Clone)]
struct Persisted {
    hotkeys: Hotkeys,
    /// `None` = the user never chose; both consumers treat that as Custom.
    mode: Option<Mode>,
    position: PanelPosition,
    local: LocalAi,
}

/// On-disk shape. `#[serde(default)]` at every level: an absent file, an
/// absent `hotkeys` object, or an absent field each fall back independently.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct SettingsFile {
    hotkeys: HotkeyEntries,
    /// `"custom"` | `"local"`. Typed as the raw string so an unrecognized
    /// value (including the removed `"default"`) falls back alone
    /// (`resolve_mode`) instead of tripping the whole-file corrupt path;
    /// omitted entirely until the user chooses.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    position: PositionEntries,
    local: LocalEntries,
    /// Top-level keys a future version wrote — preserved across saves.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct LocalEntries {
    /// Omitted while unset (the baked Ollama default applies). Kept verbatim
    /// at load — the resolver re-validates at use, and the Settings field
    /// shows the stored value so a hand-mangled one is visible and fixable.
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    vision: Option<bool>,
    /// Same forward-compat preservation, one level down.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct HotkeyEntries {
    /// Accelerator strings in the canonical `hotkey::to_accelerator` form;
    /// `None` means "use the built-in default".
    summon: Option<String>,
    capture: Option<String>,
    /// Same forward-compat preservation, one level down.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct PositionEntries {
    /// `"anchored"` | `"manual"` / an anchor's kebab string — raw so an
    /// unrecognized value falls back alone (`resolve_position`), the `mode`
    /// idiom.
    mode: Option<String>,
    anchor: Option<String>,
    /// Omitted until the panel is first dragged (or Manual first picked).
    #[serde(skip_serializing_if = "Option::is_none")]
    manual: Option<ManualPoint>,
    locked: Option<bool>,
    /// Same forward-compat preservation, one level down.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

/// The dragged spot on disk: window outer coordinates, physical
/// virtual-screen px (negative on monitors left/above the primary). `y` is
/// the pinned edge's coordinate; `edge` is raw so an unrecognized value
/// falls back alone (to `"top"`).
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
struct ManualPoint {
    x: i32,
    y: i32,
    edge: Option<String>,
}

type JsonMap = serde_json::Map<String, serde_json::Value>;
/// Unknown JSON preserved per nesting level:
/// (top-level, hotkeys, position, local).
type ExtraMaps = (JsonMap, JsonMap, JsonMap, JsonMap);

pub struct SettingsStore {
    path: PathBuf,
    state: RwLock<Persisted>,
    /// Unknown JSON preserved from the loaded file (`SettingsFile::extra`,
    /// `HotkeyEntries::extra`, `PositionEntries::extra`) — only touched by
    /// `load` and `persist`.
    extra: Mutex<ExtraMaps>,
    /// True while the frontend recorder is armed and the OS registrations are
    /// dropped (`suspend_hotkeys`/`resume_hotkeys` in `commands.rs`).
    suspended: AtomicBool,
    /// Set when the file existed but couldn't be *read* at startup (AV lock,
    /// permissions, …). Mutations are refused then — persisting would replace
    /// the user's real file with this defaults-only in-memory view.
    load_error: Option<String>,
}

impl SettingsStore {
    /// Load the store. A missing file means defaults. A corrupt file is
    /// renamed sideways to `settings.json.bak` — never silently overwritten —
    /// and defaults apply. Any *other* read failure puts the store in a
    /// refuse-mutations state instead (same rationale as `UserWikiStore`).
    /// A field that doesn't parse falls back to that field's default without
    /// rewriting the file — the next successful save repairs it.
    pub fn load(path: PathBuf) -> Self {
        let mut load_error = None;
        let file = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<SettingsFile>(&raw) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!(
                        "wikilens: {} is corrupt ({e}); keeping it as .bak and using defaults",
                        path.display()
                    );
                    let _ = fs::rename(&path, path.with_extension("json.bak"));
                    SettingsFile::default()
                }
            },
            Err(e) if e.kind() == ErrorKind::NotFound => SettingsFile::default(),
            Err(e) => {
                eprintln!(
                    "wikilens: couldn't read {} ({e}); settings are read-only this session",
                    path.display()
                );
                load_error = Some(e.to_string());
                SettingsFile::default()
            }
        };

        let mut hotkeys = Hotkeys {
            summon: resolve_field(file.hotkeys.summon, hotkey::default_summon(), "summon"),
            capture: resolve_field(file.hotkeys.capture, hotkey::default_capture(), "capture"),
        };
        // Conflict repair: the handler routes by equality, so the two roles
        // must never share a combo. Revert capture first, then summon if the
        // stored summon *was* capture's default — terminates because the two
        // defaults are disjoint (pinned in hotkey.rs tests).
        if hotkeys.summon == hotkeys.capture {
            eprintln!("wikilens: stored shortcuts collide; reverting capture to its default");
            hotkeys.capture = hotkey::default_capture();
        }
        if hotkeys.summon == hotkeys.capture {
            hotkeys.summon = hotkey::default_summon();
        }

        Self {
            path,
            state: RwLock::new(Persisted {
                hotkeys,
                mode: resolve_mode(file.mode),
                position: resolve_position(&file.position),
                local: LocalAi {
                    base_url: file
                        .local
                        .base_url
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                    vision: file.local.vision.unwrap_or(false),
                },
            }),
            extra: Mutex::new((
                file.extra,
                file.hotkeys.extra,
                file.position.extra,
                file.local.extra,
            )),
            suspended: AtomicBool::new(false),
            load_error,
        }
    }

    /// Refuse mutations when the startup read failed (see `load_error`).
    pub fn writable(&self) -> Result<(), AppError> {
        match &self.load_error {
            Some(e) => Err(AppError::Settings(format!(
                "your settings couldn't be read when WikiLens started ({e}) — restart WikiLens and try again"
            ))),
            None => Ok(()),
        }
    }

    /// Copy out the live pair (`Hotkeys` is `Copy`; the lock is held only
    /// for the read — never across a plugin call).
    pub fn hotkeys(&self) -> Hotkeys {
        self.read().hotkeys
    }

    /// The persisted mode choice; `None` = never chosen (both consumers then
    /// treat it as Custom).
    pub fn mode(&self) -> Option<Mode> {
        self.read().mode
    }

    pub fn shortcut(&self, role: HotkeyRole) -> Shortcut {
        let hk = self.hotkeys();
        match role {
            HotkeyRole::Summon => hk.summon,
            HotkeyRole::Capture => hk.capture,
        }
    }

    /// The plugin handler's matcher: which role an incoming shortcut is, if
    /// any. Parsed and constructed `Shortcut`s compare equal (the crate's
    /// `test_equality` pins this), so plain `==` is safe.
    pub fn role_of(&self, shortcut: &Shortcut) -> Option<HotkeyRole> {
        let hk = self.hotkeys();
        if *shortcut == hk.summon {
            Some(HotkeyRole::Summon)
        } else if *shortcut == hk.capture {
            Some(HotkeyRole::Capture)
        } else {
            None
        }
    }

    /// Store a new combo for a role: persisted first, committed to memory
    /// only on success. Registration is the caller's job (`commands.rs`) —
    /// this store never touches the plugin. The cross-role conflict check
    /// here is defense in depth; the command layer rejects earlier with
    /// friendlier copy.
    pub fn set_hotkey(&self, role: HotkeyRole, new: Shortcut) -> Result<(), AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = guard.clone();
        match role {
            HotkeyRole::Summon => next.hotkeys.summon = new,
            HotkeyRole::Capture => next.hotkeys.capture = new,
        }
        if next.hotkeys.summon == next.hotkeys.capture {
            return Err(AppError::Hotkey(
                "That combo is already your other shortcut — pick a different one.".to_string(),
            ));
        }
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Store the mode choice: persist-then-commit, the `set_hotkey` shape
    /// (no conflict check — any mode is valid against any other setting).
    pub fn set_mode(&self, mode: Mode) -> Result<(), AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = guard.clone();
        next.mode = Some(mode);
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// The persisted overlay placement (both memories + the padlock).
    pub fn panel_position(&self) -> PanelPosition {
        self.read().position
    }

    /// Store a placement change: persist-then-commit, the `set_mode` shape.
    /// Callers compose the whole `PanelPosition` (mode + both memories) so a
    /// pick can never clobber the other mode's remembered state by accident.
    pub fn set_panel_position(&self, position: PanelPosition) -> Result<(), AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = guard.clone();
        next.position = position;
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Compose-and-store a placement under the write lock — the
    /// `fetch_update` shape: the closure sees the current value and declines
    /// with `None` (`Ok(false)`, nothing persisted). The closure runs WHILE
    /// the write lock is held: keep it to atomic loads and pure composition —
    /// never call back into this store, never touch the window (an
    /// `apply_rect` path can re-enter the Moved handler synchronously, whose
    /// read would deadlock).
    pub fn update_panel_position(
        &self,
        f: impl FnOnce(PanelPosition) -> Option<PanelPosition>,
    ) -> Result<bool, AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = guard.clone();
        let Some(position) = f(next.position) else {
            return Ok(false);
        };
        next.position = position;
        self.persist(&next)?;
        *guard = next;
        Ok(true)
    }

    /// The Local AI config (a clone; the lock is held only for the read).
    #[allow(dead_code)] // wired up by the backend cut; the attribute dies there
    pub fn local(&self) -> LocalAi {
        self.read().local.clone()
    }

    /// Store the Local AI base URL — already normalized by the command layer
    /// (`target::normalize_local_base_url`); `None` clears back to the baked
    /// default. Persist-then-commit, the `set_mode` shape.
    #[allow(dead_code)]
    pub fn set_local_base_url(&self, base_url: Option<String>) -> Result<(), AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = guard.clone();
        next.local.base_url = base_url;
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Flip the Local AI "reads images" toggle alone — single-purpose like
    /// `set_position_locked`, so the frontend never re-sends the URL to
    /// toggle it.
    #[allow(dead_code)]
    pub fn set_local_vision(&self, vision: bool) -> Result<(), AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = guard.clone();
        next.local.vision = vision;
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Flip the padlock alone — single-purpose like `set_mode`, so the
    /// frontend never re-sends a placement to toggle it.
    pub fn set_position_locked(&self, locked: bool) -> Result<(), AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = guard.clone();
        next.position.locked = locked;
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Mark the recorder-armed state. Returns the *previous* value so callers
    /// can make suspend/resume idempotent.
    pub fn suspend(&self) -> bool {
        self.suspended.swap(true, Ordering::SeqCst)
    }

    pub fn resume(&self) -> bool {
        self.suspended.swap(false, Ordering::SeqCst)
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended.load(Ordering::SeqCst)
    }

    /// Crash-safe write: temp file in the same dir, then rename over the
    /// real one (std rename replaces existing files on Windows too).
    fn persist(&self, next: &Persisted) -> Result<(), AppError> {
        let settings_err = |e: &dyn std::fmt::Display| AppError::Settings(e.to_string());
        let dir = self
            .path
            .parent()
            .ok_or_else(|| AppError::Settings("no data directory".to_string()))?;
        fs::create_dir_all(dir).map_err(|e| settings_err(&e))?;
        let (file_extra, hotkeys_extra, position_extra, local_extra) = {
            let guard = self.extra.lock().unwrap_or_else(PoisonError::into_inner);
            guard.clone()
        };
        let file = SettingsFile {
            hotkeys: HotkeyEntries {
                summon: Some(hotkey::to_accelerator(&next.hotkeys.summon)),
                capture: Some(hotkey::to_accelerator(&next.hotkeys.capture)),
                extra: hotkeys_extra,
            },
            mode: next.mode.map(|m| m.as_str().to_string()),
            position: PositionEntries {
                mode: Some(next.position.mode.as_str().to_string()),
                anchor: Some(next.position.anchor.as_str().to_string()),
                manual: next.position.manual.map(|spot| ManualPoint {
                    x: spot.x,
                    y: spot.y,
                    edge: Some(spot.edge.as_str().to_string()),
                }),
                locked: Some(next.position.locked),
                extra: position_extra,
            },
            local: LocalEntries {
                base_url: next.local.base_url.clone(),
                vision: Some(next.local.vision),
                extra: local_extra,
            },
            extra: file_extra,
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| settings_err(&e))?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| settings_err(&e))?;
        fs::rename(&tmp, &self.path).map_err(|e| settings_err(&e))
    }

    // Poisoning: same policy as UserWikiStore — plain data, safe to keep
    // using after a panicked writer.
    fn read(&self) -> RwLockReadGuard<'_, Persisted> {
        self.state.read().unwrap_or_else(PoisonError::into_inner)
    }
    fn write(&self) -> RwLockWriteGuard<'_, Persisted> {
        self.state.write().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The stored mode, or `None` ("never chosen") when absent or unrecognized —
/// like `resolve_field`, the file is not rewritten; the next save repairs it.
/// A stored `"default"` is the removed Built In mode: it degrades the same
/// way (both consumers treat `None` as Custom), with a notice naming the
/// removal instead of the generic invalid-value line.
fn resolve_mode(stored: Option<String>) -> Option<Mode> {
    let s = stored?;
    let mode = Mode::parse(&s);
    if mode.is_none() {
        if s == "default" {
            eprintln!(
                "wikilens: Default mode was removed — pick Custom API or Local AI \
                 in Settings → Answers"
            );
        } else {
            eprintln!("wikilens: stored mode {s:?} is invalid; treating it as unchosen");
        }
    }
    mode
}

/// The stored placement, each field falling back alone (the file is never
/// rewritten by load — the next save repairs it). An unrecognized `mode` or
/// `anchor` string reverts to that field's default; a wrong-*typed* field
/// trips the whole-file corrupt path like every other setting.
fn resolve_position(stored: &PositionEntries) -> PanelPosition {
    let defaults = PanelPosition::default();
    let mode = match &stored.mode {
        None => defaults.mode,
        Some(s) => PositionMode::parse(s).unwrap_or_else(|| {
            eprintln!("wikilens: stored position mode {s:?} is invalid; using the default");
            defaults.mode
        }),
    };
    let anchor = match &stored.anchor {
        None => defaults.anchor,
        Some(s) => PanelAnchor::parse(s).unwrap_or_else(|| {
            eprintln!("wikilens: stored anchor {s:?} is invalid; using the default");
            defaults.anchor
        }),
    };
    let manual = stored.manual.as_ref().map(|p| ManualSpot {
        x: p.x,
        y: p.y,
        edge: match &p.edge {
            None => ManualEdge::Top,
            Some(s) => ManualEdge::parse(s).unwrap_or_else(|| {
                eprintln!("wikilens: stored manual edge {s:?} is invalid; using top");
                ManualEdge::Top
            }),
        },
    });
    PanelPosition {
        mode,
        anchor,
        manual,
        locked: stored.locked.unwrap_or(defaults.locked),
    }
}

/// A stored accelerator, or the role's default when absent/unparseable.
fn resolve_field(stored: Option<String>, default: Shortcut, name: &str) -> Shortcut {
    match stored {
        None => default,
        Some(s) => match hotkey::parse_accelerator(&s) {
            Ok(shortcut) => shortcut,
            Err(_) => {
                eprintln!("wikilens: stored {name} shortcut {s:?} is invalid; using the default");
                default
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alt_q() -> Shortcut {
        hotkey::parse_accelerator("Alt+KeyQ").unwrap()
    }

    #[test]
    fn missing_file_serves_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"));
        assert_eq!(store.hotkeys().summon, hotkey::default_summon());
        assert_eq!(store.hotkeys().capture, hotkey::default_capture());
        assert_eq!(
            store.role_of(&hotkey::default_summon()),
            Some(HotkeyRole::Summon)
        );
        assert_eq!(
            store.role_of(&hotkey::default_capture()),
            Some(HotkeyRole::Capture)
        );
        assert_eq!(store.role_of(&alt_q()), None);
        assert_eq!(store.mode(), None);
    }

    #[test]
    fn set_mode_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.mode(), None);
        store.set_mode(Mode::Custom).unwrap();
        assert_eq!(store.mode(), Some(Mode::Custom));

        let reloaded = SettingsStore::load(path.clone());
        assert_eq!(reloaded.mode(), Some(Mode::Custom));

        // The file holds the wire string — pinned so a future serde derive
        // on `Mode` can't drift from `as_str`.
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"mode\": \"custom\""), "raw file was: {raw}");
    }

    /// The IPC encoding (serde derive) must match the file encoding (`as_str`)
    /// byte for byte — `set_mode_persists_and_reloads` pins the file half.
    #[test]
    fn mode_serde_matches_the_stored_wire_strings() {
        assert_eq!(serde_json::to_value(Mode::Custom).unwrap(), "custom");
        assert_eq!(serde_json::to_value(Mode::Local).unwrap(), "local");
        assert_eq!(
            serde_json::from_value::<Mode>("local".into()).unwrap(),
            Mode::Local
        );
        assert!(serde_json::from_value::<Mode>("banana".into()).is_err());
        // The removed Built In wire string must not round-trip anymore — an
        // old frontend build sending it gets a serde error, not a ghost mode.
        assert!(serde_json::from_value::<Mode>("default".into()).is_err());
    }

    #[test]
    fn invalid_stored_mode_falls_back_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{ "hotkeys": { "capture": "Alt+KeyQ" }, "mode": "banana" }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.mode(), None);
        assert_eq!(store.shortcut(HotkeyRole::Capture), alt_q());
        // The file is not rewritten by load — repair happens on the next save.
        assert!(fs::read_to_string(&path).unwrap().contains("banana"));
    }

    /// The migration path for pre-0.2.0 installs: a stored `"default"` (the
    /// removed Built In mode) degrades to unchosen — treated as Custom by
    /// both consumers — without touching the rest of the file.
    #[test]
    fn stored_default_mode_degrades_to_unchosen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{ "hotkeys": { "capture": "Alt+KeyQ" }, "mode": "default" }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.mode(), None);
        assert_eq!(store.shortcut(HotkeyRole::Capture), alt_q());
        // A later mode pick repairs the file to a live wire string.
        store.set_mode(Mode::Local).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("\"mode\": \"local\""));
    }

    #[test]
    fn set_panel_position_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.panel_position(), PanelPosition::default());
        let spot = ManualSpot {
            x: -8,
            y: 1240,
            edge: ManualEdge::Bottom,
        };
        store
            .set_panel_position(PanelPosition {
                mode: PositionMode::Manual,
                anchor: PanelAnchor::BottomLeft,
                manual: Some(spot),
                locked: false,
            })
            .unwrap();

        let reloaded = SettingsStore::load(path.clone());
        let position = reloaded.panel_position();
        assert_eq!(position.mode, PositionMode::Manual);
        assert_eq!(position.anchor, PanelAnchor::BottomLeft);
        assert_eq!(position.manual, Some(spot));
        assert!(!position.locked);

        // The file holds the wire strings — pinned like `mode`'s so the serde
        // derives can't drift from `as_str`.
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"mode\": \"manual\""), "raw file was: {raw}");
        assert!(
            raw.contains("\"anchor\": \"bottom-left\""),
            "raw file was: {raw}"
        );
        assert!(raw.contains("\"x\": -8"), "raw file was: {raw}");
        assert!(raw.contains("\"y\": 1240"), "raw file was: {raw}");
        assert!(raw.contains("\"edge\": \"bottom\""), "raw file was: {raw}");
    }

    #[test]
    fn set_position_locked_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        assert!(!store.panel_position().locked);
        store.set_position_locked(true).unwrap();
        assert!(store.panel_position().locked);
        // The lock flips alone — placement and memories untouched.
        assert_eq!(store.panel_position().mode, PositionMode::Anchored);

        let reloaded = SettingsStore::load(path.clone());
        assert!(reloaded.panel_position().locked);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"locked\": true"), "raw file was: {raw}");
    }

    #[test]
    fn update_panel_position_commits_and_composes_from_stored_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        // The deterministic Race-B shape: the padlock flips first, then the
        // settle composes — the closure must see the flipped value and spread
        // it, never a stale pre-flip read.
        store.set_position_locked(true).unwrap();
        // Captured OUTSIDE the closure — reading the store from inside it
        // would be the very same-thread deadlock the method's doc forbids.
        let stored = store.panel_position();
        let spot = ManualSpot {
            x: 120,
            y: 640,
            edge: ManualEdge::Top,
        };
        let updated = store
            .update_panel_position(|current| {
                assert_eq!(current, stored);
                Some(PanelPosition {
                    mode: PositionMode::Manual,
                    manual: Some(spot),
                    ..current
                })
            })
            .unwrap();
        assert!(updated);

        let position = store.panel_position();
        assert_eq!(position.mode, PositionMode::Manual);
        assert_eq!(position.manual, Some(spot));
        assert!(position.locked, "the earlier padlock flip must survive");

        let reloaded = SettingsStore::load(path);
        assert_eq!(reloaded.panel_position(), position);
    }

    #[test]
    fn update_panel_position_none_declines_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        let before = store.panel_position();
        let updated = store.update_panel_position(|_| None).unwrap();
        assert!(!updated);
        assert_eq!(store.panel_position(), before);
        assert!(!path.exists(), "a decline must not create the file");
    }

    /// The IPC encodings (serde derives) must match the file encodings
    /// (`as_str`) byte for byte — the round-trip tests pin the file half.
    #[test]
    fn panel_anchor_serde_matches_the_stored_wire_strings() {
        for (anchor, wire) in [
            (PanelAnchor::TopRight, "top-right"),
            (PanelAnchor::TopLeft, "top-left"),
            (PanelAnchor::BottomRight, "bottom-right"),
            (PanelAnchor::BottomLeft, "bottom-left"),
            (PanelAnchor::Center, "center"),
        ] {
            assert_eq!(serde_json::to_value(anchor).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<PanelAnchor>(wire.into()).unwrap(),
                anchor
            );
            assert_eq!(anchor.as_str(), wire);
            assert_eq!(PanelAnchor::parse(wire), Some(anchor));
        }
        assert!(serde_json::from_value::<PanelAnchor>("under-the-couch".into()).is_err());
    }

    #[test]
    fn position_mode_serde_matches_the_stored_wire_strings() {
        assert_eq!(
            serde_json::to_value(PositionMode::Anchored).unwrap(),
            "anchored"
        );
        assert_eq!(serde_json::to_value(PositionMode::Manual).unwrap(), "manual");
        assert_eq!(
            serde_json::from_value::<PositionMode>("manual".into()).unwrap(),
            PositionMode::Manual
        );
        assert!(serde_json::from_value::<PositionMode>("free".into()).is_err());
    }

    #[test]
    fn manual_edge_serde_matches_the_stored_wire_strings() {
        assert_eq!(serde_json::to_value(ManualEdge::Top).unwrap(), "top");
        assert_eq!(serde_json::to_value(ManualEdge::Bottom).unwrap(), "bottom");
        assert_eq!(
            serde_json::from_value::<ManualEdge>("bottom".into()).unwrap(),
            ManualEdge::Bottom
        );
        assert!(serde_json::from_value::<ManualEdge>("sideways".into()).is_err());
        assert_eq!(ManualEdge::parse("top"), Some(ManualEdge::Top));
        assert_eq!(ManualEdge::Bottom.as_str(), "bottom");
    }

    #[test]
    fn invalid_stored_position_falls_back_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "hotkeys": { "capture": "Alt+KeyQ" },
                "position": { "mode": "banana", "anchor": "under-the-couch", "manual": { "x": 4, "y": 9, "edge": "sideways" } }
            }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path.clone());
        // Each bad field reverts alone; the valid ones survive.
        let position = store.panel_position();
        assert_eq!(position.mode, PositionMode::Anchored);
        assert_eq!(position.anchor, PanelAnchor::TopRight);
        assert_eq!(
            position.manual,
            Some(ManualSpot {
                x: 4,
                y: 9,
                edge: ManualEdge::Top,
            })
        );
        assert!(!position.locked);
        assert_eq!(store.shortcut(HotkeyRole::Capture), alt_q());
        // The file is not rewritten by load — repair happens on the next save.
        assert!(fs::read_to_string(&path).unwrap().contains("banana"));
    }

    #[test]
    fn mode_and_hotkeys_survive_each_others_saves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        store.set_mode(Mode::Local).unwrap();
        store.set_hotkey(HotkeyRole::Summon, alt_q()).unwrap();

        let reloaded = SettingsStore::load(path.clone());
        assert_eq!(reloaded.mode(), Some(Mode::Local));
        assert_eq!(reloaded.shortcut(HotkeyRole::Summon), alt_q());

        reloaded.set_mode(Mode::Custom).unwrap();
        let again = SettingsStore::load(path);
        assert_eq!(again.shortcut(HotkeyRole::Summon), alt_q());
        assert_eq!(again.mode(), Some(Mode::Custom));
    }

    #[test]
    fn set_hotkey_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        store.set_hotkey(HotkeyRole::Summon, alt_q()).unwrap();
        assert_eq!(store.shortcut(HotkeyRole::Summon), alt_q());

        // A fresh load sees the stored summon; capture stays default.
        let reloaded = SettingsStore::load(path.clone());
        assert_eq!(reloaded.shortcut(HotkeyRole::Summon), alt_q());
        assert_eq!(
            reloaded.shortcut(HotkeyRole::Capture),
            hotkey::default_capture()
        );

        // The file holds canonical accelerator strings.
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"Alt+KeyQ\""), "raw file was: {raw}");
        assert!(raw.contains("\"Ctrl+Shift+KeyC\""), "raw file was: {raw}");
    }

    #[test]
    fn setting_the_other_roles_combo_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"));
        let err = store
            .set_hotkey(HotkeyRole::Summon, hotkey::default_capture())
            .unwrap_err();
        assert!(matches!(err, AppError::Hotkey(_)));
        assert_eq!(store.shortcut(HotkeyRole::Summon), hotkey::default_summon());
    }

    #[test]
    fn invalid_stored_field_falls_back_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{ "hotkeys": { "summon": "not a key", "capture": "Alt+KeyQ" } }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.shortcut(HotkeyRole::Summon), hotkey::default_summon());
        assert_eq!(store.shortcut(HotkeyRole::Capture), alt_q());
        // The file is not rewritten by load — repair happens on the next save.
        assert!(fs::read_to_string(&path).unwrap().contains("not a key"));
    }

    #[test]
    fn colliding_stored_combos_revert_capture_then_summon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        // Both stored as the same combo → capture reverts to its default.
        fs::write(
            &path,
            r#"{ "hotkeys": { "summon": "Alt+KeyQ", "capture": "Alt+KeyQ" } }"#,
        )
        .unwrap();
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.shortcut(HotkeyRole::Summon), alt_q());
        assert_eq!(
            store.shortcut(HotkeyRole::Capture),
            hotkey::default_capture()
        );

        // Stored summon IS capture's default → both roles end on defaults.
        fs::write(
            &path,
            r#"{ "hotkeys": { "summon": "Ctrl+Shift+KeyC" } }"#,
        )
        .unwrap();
        let store = SettingsStore::load(path);
        assert_eq!(store.shortcut(HotkeyRole::Summon), hotkey::default_summon());
        assert_eq!(
            store.shortcut(HotkeyRole::Capture),
            hotkey::default_capture()
        );
    }

    #[test]
    fn corrupt_file_is_backed_up_not_clobbered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "definitely not json").unwrap();

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.hotkeys().summon, hotkey::default_summon());

        let bak = fs::read_to_string(dir.path().join("settings.json.bak")).unwrap();
        assert_eq!(bak, "definitely not json");
        assert!(!path.exists(), "corrupt file should have been renamed away");
    }

    #[test]
    fn unreadable_file_refuses_mutations_instead_of_clobbering() {
        // A directory at the store path makes read_to_string fail with a
        // non-NotFound error — the portable stand-in for an AV/permission
        // lock. The store must refuse mutations, not persist its defaults.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::create_dir(&path).unwrap();

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.hotkeys().summon, hotkey::default_summon());
        assert!(matches!(
            store.set_hotkey(HotkeyRole::Summon, alt_q()),
            Err(AppError::Settings(_))
        ));
        assert!(matches!(
            store.set_mode(Mode::Custom),
            Err(AppError::Settings(_))
        ));
        assert!(matches!(
            store.set_panel_position(PanelPosition {
                mode: PositionMode::Manual,
                manual: Some(ManualSpot {
                    x: 10,
                    y: 10,
                    edge: ManualEdge::Top,
                }),
                ..PanelPosition::default()
            }),
            Err(AppError::Settings(_))
        ));
        assert!(matches!(
            store.set_position_locked(true),
            Err(AppError::Settings(_))
        ));
        // `writable()` gates BEFORE the closure runs — the panic proves it.
        assert!(matches!(
            store.update_panel_position(|_| panic!("must not run")),
            Err(AppError::Settings(_))
        ));
        assert!(path.is_dir(), "store path must not have been touched");
    }

    #[test]
    fn persist_preserves_unknown_keys_from_future_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "future_panel": { "layout": "wide" },
                "hotkeys": { "summon": "Alt+KeyQ", "push_to_talk": "F13" },
                "position": { "anchor": "center", "future_snap": "edges" },
                "local": { "vision": true, "future_warmup": "eager" }
            }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path.clone());
        store.set_hotkey(HotkeyRole::Capture, alt_p()).unwrap();
        store.set_mode(Mode::Custom).unwrap();
        store.set_position_locked(true).unwrap();

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["future_panel"]["layout"], "wide");
        assert_eq!(saved["hotkeys"]["push_to_talk"], "F13");
        assert_eq!(saved["hotkeys"]["summon"], "Alt+KeyQ");
        assert_eq!(saved["hotkeys"]["capture"], "Alt+KeyP");
        assert_eq!(saved["mode"], "custom");
        assert_eq!(saved["position"]["future_snap"], "edges");
        assert_eq!(saved["position"]["anchor"], "center");
        assert_eq!(saved["position"]["locked"], true);
        assert_eq!(saved["local"]["future_warmup"], "eager");
        assert_eq!(saved["local"]["vision"], true);
    }

    #[test]
    fn local_config_defaults_and_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        assert_eq!(store.local(), LocalAi::default(), "unset = default+off");

        store
            .set_local_base_url(Some("http://box.lan:8080/v1".to_string()))
            .unwrap();
        store.set_local_vision(true).unwrap();
        assert_eq!(
            store.local(),
            LocalAi {
                base_url: Some("http://box.lan:8080/v1".to_string()),
                vision: true,
            }
        );

        let reloaded = SettingsStore::load(path.clone());
        assert_eq!(reloaded.local(), store.local());

        // Clearing drops the key from the file entirely (skip_serializing_if),
        // so an unset URL and a never-set URL are the same on disk.
        store.set_local_base_url(None).unwrap();
        assert_eq!(store.local().base_url, None);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("base_url"), "raw file was: {raw}");
        assert!(raw.contains("\"vision\": true"), "raw file was: {raw}");
    }

    /// A hand-edited blank/whitespace base_url loads as "unset" instead of
    /// producing an empty-string base the resolver would have to special-case.
    #[test]
    fn blank_stored_base_url_loads_as_unset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{ "local": { "base_url": "   " } }"#).unwrap();
        let store = SettingsStore::load(path);
        assert_eq!(store.local().base_url, None);
        assert!(!store.local().vision, "vision defaults off");
    }

    fn alt_p() -> Shortcut {
        hotkey::parse_accelerator("Alt+KeyP").unwrap()
    }

    #[test]
    fn suspend_and_resume_report_the_previous_state() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"));
        assert!(!store.is_suspended());
        assert!(!store.suspend(), "first suspend: was not suspended");
        assert!(store.suspend(), "second suspend: already suspended");
        assert!(store.is_suspended());
        assert!(store.resume(), "first resume: was suspended");
        assert!(!store.resume(), "second resume: already resumed");
        assert!(!store.is_suspended());
    }
}
