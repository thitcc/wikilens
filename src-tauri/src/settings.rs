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

/// Which model-source mode the player chose in the config panel — `"default"`
/// / `"custom"` on disk (vault/2026-07-26_default-mode-and-byo-api-keys.md).
/// No serde derives yet: phase 2 adds them when `SettingsInfo` grows the
/// field; until then the store writes the strings via `as_str`, pinned by
/// `set_mode_persists_and_reloads` so the two encodings can't drift.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Default,
    Custom,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Default => "default",
            Mode::Custom => "custom",
        }
    }

    fn parse(s: &str) -> Option<Mode> {
        match s {
            "default" => Some(Mode::Default),
            "custom" => Some(Mode::Custom),
            _ => None,
        }
    }
}

/// Everything `persist` writes, mutated as one candidate (clone-mutate-
/// persist-commit). One write lock held across persist serializes every
/// mutation, so two concurrent mutators can never save each other's state
/// stale.
#[derive(Clone, Copy)]
struct Persisted {
    hotkeys: Hotkeys,
    /// `None` = the user never chose; phase 3 auto-senses on first launch.
    mode: Option<Mode>,
}

/// On-disk shape. `#[serde(default)]` at every level: an absent file, an
/// absent `hotkeys` object, or an absent field each fall back independently.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct SettingsFile {
    hotkeys: HotkeyEntries,
    /// `"default"` | `"custom"`. Typed as the raw string so an unrecognized
    /// value falls back alone (`resolve_mode`) instead of tripping the
    /// whole-file corrupt path; omitted entirely until the user chooses.
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    /// Top-level keys a future version wrote — preserved across saves.
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

pub struct SettingsStore {
    path: PathBuf,
    state: RwLock<Persisted>,
    /// Unknown JSON preserved from the loaded file (`SettingsFile::extra`,
    /// `HotkeyEntries::extra`) — only touched by `load` and `persist`.
    extra: Mutex<(
        serde_json::Map<String, serde_json::Value>,
        serde_json::Map<String, serde_json::Value>,
    )>,
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
            }),
            extra: Mutex::new((file.extra, file.hotkeys.extra)),
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

    /// Copy out the live pair (`Persisted` is `Copy`; the lock is held only
    /// for the read — never across a plugin call).
    pub fn hotkeys(&self) -> Hotkeys {
        self.read().hotkeys
    }

    /// The persisted mode choice; `None` = never chosen (phase 3 auto-senses
    /// on first launch). Consumed by phase 2's `get_settings`; the allow dies
    /// with it.
    #[allow(dead_code)]
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
        let mut next = *guard;
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
    /// Consumed by phase 2's `set_mode` command; the allow dies with it.
    #[allow(dead_code)]
    pub fn set_mode(&self, mode: Mode) -> Result<(), AppError> {
        self.writable()?;
        let mut guard = self.write();
        let mut next = *guard;
        next.mode = Some(mode);
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
        let (file_extra, hotkeys_extra) = {
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
fn resolve_mode(stored: Option<String>) -> Option<Mode> {
    let s = stored?;
    let mode = Mode::parse(&s);
    if mode.is_none() {
        eprintln!("wikilens: stored mode {s:?} is invalid; treating it as unchosen");
    }
    mode
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

    #[test]
    fn mode_and_hotkeys_survive_each_others_saves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");

        let store = SettingsStore::load(path.clone());
        store.set_mode(Mode::Default).unwrap();
        store.set_hotkey(HotkeyRole::Summon, alt_q()).unwrap();

        let reloaded = SettingsStore::load(path.clone());
        assert_eq!(reloaded.mode(), Some(Mode::Default));
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
                "hotkeys": { "summon": "Alt+KeyQ", "push_to_talk": "F13" }
            }"#,
        )
        .unwrap();

        let store = SettingsStore::load(path.clone());
        store.set_hotkey(HotkeyRole::Capture, alt_p()).unwrap();
        store.set_mode(Mode::Custom).unwrap();

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["future_panel"]["layout"], "wide");
        assert_eq!(saved["hotkeys"]["push_to_talk"], "F13");
        assert_eq!(saved["hotkeys"]["summon"], "Alt+KeyQ");
        assert_eq!(saved["hotkeys"]["capture"], "Alt+KeyP");
        assert_eq!(saved["mode"], "custom");
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
