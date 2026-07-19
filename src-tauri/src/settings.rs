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

/// On-disk shape. `#[serde(default)]` at every level: an absent file, an
/// absent `hotkeys` object, or an absent field each fall back independently.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct SettingsFile {
    hotkeys: HotkeyEntries,
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
    hotkeys: RwLock<Hotkeys>,
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
            hotkeys: RwLock::new(hotkeys),
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

    /// Copy out the live pair (`Shortcut` is `Copy`; the lock is held only
    /// for the read — never across a plugin call).
    pub fn hotkeys(&self) -> Hotkeys {
        *self.read()
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
            HotkeyRole::Summon => next.summon = new,
            HotkeyRole::Capture => next.capture = new,
        }
        if next.summon == next.capture {
            return Err(AppError::Hotkey(
                "That combo is already your other shortcut — pick a different one.".to_string(),
            ));
        }
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
    fn persist(&self, next: &Hotkeys) -> Result<(), AppError> {
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
                summon: Some(hotkey::to_accelerator(&next.summon)),
                capture: Some(hotkey::to_accelerator(&next.capture)),
                extra: hotkeys_extra,
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
    fn read(&self) -> RwLockReadGuard<'_, Hotkeys> {
        self.hotkeys.read().unwrap_or_else(PoisonError::into_inner)
    }
    fn write(&self) -> RwLockWriteGuard<'_, Hotkeys> {
        self.hotkeys.write().unwrap_or_else(PoisonError::into_inner)
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

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["future_panel"]["layout"], "wide");
        assert_eq!(saved["hotkeys"]["push_to_talk"], "F13");
        assert_eq!(saved["hotkeys"]["summon"], "Alt+KeyQ");
        assert_eq!(saved["hotkeys"]["capture"], "Alt+KeyP");
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
