//! Persistent store for panel-entered LLM API keys.
//!
//! Backing file: `keys.json` in the Tauri app-data dir, next to
//! `settings.json` and `wikis.json`; managed as Tauri state from `lib.rs`'s
//! setup. Values are per-user DPAPI ciphertexts (`CryptProtectData`),
//! base64-encoded — cleartext never lands on disk (pinned by
//! `raw_file_never_contains_the_key`), and a blob another Windows user or
//! machine can't decrypt decays to "no key" instead of erroring
//! (ADR: vault/2026-07-26_api-key-storage-dpapi.md). Mutations follow the
//! `UserWikiStore`/`SettingsStore` discipline: persisted to disk before they
//! become visible in memory.

// Phase 1 of vault/2026-07-26_default-mode-and-byo-api-keys.md lands this
// store behaviorally inert; the phase-2 IPC commands (set_api_key /
// remove_api_key / list_key_status) are its production callers. DELETE this
// allow when phase 2 wires them up.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// The key-store seam: the phase-2 commands and phase-3 ask-time resolver
/// consume this, with the test-only `InMemoryKeyStore` standing in for the
/// DPAPI store in their tests.
pub trait KeyStore {
    /// Store (or replace) a provider's key. Cleartext never reaches disk —
    /// implementations encrypt before persisting. Empty/whitespace keys are
    /// rejected here as defense in depth; the command layer validates earlier
    /// with friendlier copy.
    fn set(&self, provider_id: &str, key: &str) -> Result<(), AppError>;

    /// Drop a provider's entry. Removing an absent key is a successful no-op
    /// (a deliberate departure from `UserWikiStore::remove`): the caller's
    /// intent — "no stored key" — is already satisfied, presence can be
    /// legitimately stale (a ghost blob from another machine), and a
    /// double-clicked Remove erroring would be pure noise.
    fn remove(&self, provider_id: &str) -> Result<(), AppError>;

    /// Entry presence, answered without decrypting (see the ADR). May say
    /// `true` for a blob `get` can't decrypt — the panel then shows "key set"
    /// until the user removes or replaces it.
    fn has_key(&self, provider_id: &str) -> bool;

    /// The decrypted key, or `None` when unset *or* undecryptable — a
    /// migrated/corrupted blob decays to "no key", never an error.
    fn get(&self, provider_id: &str) -> Option<String>;
}

const KEYS_FILE_VERSION: u32 = 1;

/// On-disk shape: `{"version":1,"keys":{"<provider_id>":"<base64 DPAPI
/// ciphertext>"}}`. `#[serde(default)]` fills absent fields from `Default`
/// (version 1, empty map), so a hand-trimmed file still loads.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct KeysFile {
    version: u32,
    keys: BTreeMap<String, String>,
    /// Top-level keys a future version wrote — preserved across saves.
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for KeysFile {
    fn default() -> Self {
        Self {
            version: KEYS_FILE_VERSION,
            keys: BTreeMap::new(),
            extra: serde_json::Map::new(),
        }
    }
}

/// In-memory state, cloned as the mutation candidate (the `UserWikiStore`
/// clone-mutate-persist-commit pattern).
#[derive(Clone, Default)]
struct KeysState {
    keys: BTreeMap<String, String>,
    extra: serde_json::Map<String, serde_json::Value>,
}

/// The shipping `KeyStore`. Registry-agnostic — validating provider ids
/// against `providers.rs` is the command layer's job (the `UserWikiStore`
/// precedent).
pub struct DpapiKeyStore {
    path: PathBuf,
    state: RwLock<KeysState>,
    /// Set when the file existed but couldn't be *read* at startup (AV lock,
    /// permissions, a newer file version). Mutations are refused then —
    /// persisting would replace the user's real keys with this empty view.
    load_error: Option<String>,
}

impl DpapiKeyStore {
    /// Load the store. A missing file is an empty store. A corrupt file is
    /// renamed sideways to `keys.json.bak` — never silently overwritten — and
    /// the store starts empty. Any *other* read failure puts the store in a
    /// refuse-mutations state instead — as does a `version` this WikiLens
    /// doesn't understand: a valid newer file must never be renamed away or
    /// rewritten as version 1, so its keys read as unset and mutations are
    /// refused for the session.
    pub fn load(path: PathBuf) -> Self {
        let mut load_error = None;
        let file = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<KeysFile>(&raw) {
                Ok(file) if file.version == KEYS_FILE_VERSION => file,
                Ok(file) => {
                    eprintln!(
                        "wikilens: {} is version {}, but this WikiLens only understands version {KEYS_FILE_VERSION}; API keys are read-only this session",
                        path.display(),
                        file.version
                    );
                    load_error = Some(format!(
                        "it was written by a newer WikiLens (version {})",
                        file.version
                    ));
                    KeysFile::default()
                }
                Err(e) => {
                    eprintln!(
                        "wikilens: {} is corrupt ({e}); keeping it as .bak and starting empty",
                        path.display()
                    );
                    let _ = fs::rename(&path, path.with_extension("json.bak"));
                    KeysFile::default()
                }
            },
            Err(e) if e.kind() == ErrorKind::NotFound => KeysFile::default(),
            Err(e) => {
                eprintln!(
                    "wikilens: couldn't read {} ({e}); API keys are read-only this session",
                    path.display()
                );
                load_error = Some(e.to_string());
                KeysFile::default()
            }
        };
        Self {
            path,
            state: RwLock::new(KeysState {
                keys: file.keys,
                extra: file.extra,
            }),
            load_error,
        }
    }

    /// Refuse mutations when the startup read failed (see `load_error`).
    fn guard_writable(&self) -> Result<(), AppError> {
        match &self.load_error {
            Some(e) => Err(AppError::Keys(format!(
                "your API keys couldn't be read when WikiLens started ({e}) — restart WikiLens and try again"
            ))),
            None => Ok(()),
        }
    }

    /// Crash-safe write: temp file in the same dir, then rename over the
    /// real one (std rename replaces existing files on Windows too).
    fn persist(&self, next: &KeysState) -> Result<(), AppError> {
        let keys_err = |e: &dyn std::fmt::Display| AppError::Keys(e.to_string());
        let dir = self
            .path
            .parent()
            .ok_or_else(|| AppError::Keys("no data directory".to_string()))?;
        fs::create_dir_all(dir).map_err(|e| keys_err(&e))?;
        let file = KeysFile {
            version: KEYS_FILE_VERSION,
            keys: next.keys.clone(),
            extra: next.extra.clone(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| keys_err(&e))?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| keys_err(&e))?;
        fs::rename(&tmp, &self.path).map_err(|e| keys_err(&e))
    }

    // Poisoning: same policy as the other stores — plain data, safe to keep
    // using after a panicked writer.
    fn read(&self) -> RwLockReadGuard<'_, KeysState> {
        self.state.read().unwrap_or_else(PoisonError::into_inner)
    }
    fn write(&self) -> RwLockWriteGuard<'_, KeysState> {
        self.state.write().unwrap_or_else(PoisonError::into_inner)
    }
}

impl KeyStore for DpapiKeyStore {
    fn set(&self, provider_id: &str, key: &str) -> Result<(), AppError> {
        self.guard_writable()?;
        if key.trim().is_empty() {
            return Err(AppError::Keys("an API key can't be empty".to_string()));
        }
        // Encrypt before taking the lock — DPAPI is an out-of-process call.
        let ciphertext = protect(key.as_bytes()).map_err(AppError::Keys)?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(ciphertext);
        let mut guard = self.write();
        let mut next = guard.clone();
        next.keys.insert(provider_id.to_string(), encoded);
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    fn remove(&self, provider_id: &str) -> Result<(), AppError> {
        self.guard_writable()?;
        let mut guard = self.write();
        if !guard.keys.contains_key(provider_id) {
            // Already absent — the no-op leg of the trait contract; the disk
            // is deliberately untouched (a fresh store never creates a file).
            return Ok(());
        }
        let mut next = guard.clone();
        next.keys.remove(provider_id);
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    fn has_key(&self, provider_id: &str) -> bool {
        self.read().keys.contains_key(provider_id)
    }

    fn get(&self, provider_id: &str) -> Option<String> {
        let encoded = self.read().keys.get(provider_id).cloned()?;
        // Decode + decrypt outside the lock; every failure decays to "no key"
        // (the ADR's migrated-blob rule). Diagnostics carry the provider id
        // and OS error text only — never key material.
        let ciphertext = match base64::engine::general_purpose::STANDARD.decode(&encoded) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!(
                    "wikilens: stored key for {provider_id} isn't valid base64 ({e}); treating it as unset"
                );
                return None;
            }
        };
        match unprotect(&ciphertext) {
            Some(bytes) => String::from_utf8(bytes).ok(),
            None => {
                eprintln!(
                    "wikilens: stored key for {provider_id} couldn't be decrypted (wrong Windows user or machine, or a corrupted entry); treating it as unset"
                );
                None
            }
        }
    }
}

/// DPAPI-encrypt for the current Windows user. `Err` carries the OS error
/// text (`io::Error::last_os_error()`) — never the input bytes.
///
/// Null description/entropy/prompt: per-user scope is DPAPI's default, and
/// app-supplied entropy adds nothing against a same-user process while
/// creating one more thing a migration can lose. `UI_FORBIDDEN` keeps DPAPI
/// from ever popping a dialog under the overlay.
#[cfg(windows)]
fn protect(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    // `set` rejects empty keys before calling; belt-and-braces so an empty
    // slice's dangling pointer never reaches the FFI boundary.
    if plaintext.is_empty() {
        return Err("nothing to encrypt".to_string());
    }
    let input = CRYPT_INTEGER_BLOB {
        // Keys are tiny; the u32 cast can't truncate.
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // A successful call owns pbData: copy then LocalFree in straight-line
    // code, with no early return between them.
    let bytes = unsafe {
        let copied = (!output.pbData.is_null() && output.cbData != 0)
            .then(|| std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec());
        if !output.pbData.is_null() {
            LocalFree(output.pbData.cast());
        }
        copied
    };
    bytes.ok_or_else(|| "DPAPI returned an empty blob".to_string())
}

/// Decrypt a DPAPI blob, or `None` on any failure (wrong user/machine,
/// corrupt blob, degenerate input). Never key material in diagnostics —
/// callers add their own context.
#[cfg(windows)]
fn unprotect(ciphertext: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    // A hand-edited file can hold base64 of "" — don't hand DPAPI an empty
    // slice's dangling pointer.
    if ciphertext.is_empty() {
        return None;
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: ciphertext.len() as u32,
        pbData: ciphertext.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // Null out-description: we never set one in `protect`.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return None;
    }
    unsafe {
        let copied = (!output.pbData.is_null() && output.cbData != 0)
            .then(|| std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec());
        if !output.pbData.is_null() {
            LocalFree(output.pbData.cast());
        }
        copied
    }
}

// Honest non-Windows stubs (the window.rs FFI precedent): the crate keeps its
// "compiles everywhere, works on Windows" property, and there is deliberately
// no plaintext fallback.
#[cfg(not(windows))]
fn protect(_plaintext: &[u8]) -> Result<Vec<u8>, String> {
    Err("storing API keys needs Windows (DPAPI)".to_string())
}
#[cfg(not(windows))]
fn unprotect(_ciphertext: &[u8]) -> Option<Vec<u8>> {
    None
}

/// Test-only `KeyStore`: cleartext in memory, nothing on disk, no DPAPI.
/// Lives here (not `test_support.rs`) for cohesion with the trait; later
/// phases' command tests reuse it as-is.
#[cfg(test)]
#[derive(Default)]
pub struct InMemoryKeyStore {
    keys: std::sync::Mutex<BTreeMap<String, String>>,
}

#[cfg(test)]
impl KeyStore for InMemoryKeyStore {
    fn set(&self, provider_id: &str, key: &str) -> Result<(), AppError> {
        if key.trim().is_empty() {
            return Err(AppError::Keys("an API key can't be empty".to_string()));
        }
        self.keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(provider_id.to_string(), key.to_string());
        Ok(())
    }

    fn remove(&self, provider_id: &str) -> Result<(), AppError> {
        self.keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(provider_id);
        Ok(())
    }

    fn has_key(&self, provider_id: &str) -> bool {
        self.keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(provider_id)
    }

    fn get(&self, provider_id: &str) -> Option<String> {
        self.keys
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(provider_id)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recognizable stand-in that must never appear on disk. Long enough
    /// that a substring hit can't be coincidence.
    const SENTINEL: &str = "WIKILENS-SENTINEL-NOT-A-REAL-KEY";

    /// The trait semantics, run against both impls so they can't fork.
    fn assert_keystore_contract(store: &impl KeyStore) {
        assert!(!store.has_key("anthropic"));
        assert!(store.get("anthropic").is_none());

        store.set("anthropic", "sk-first").unwrap();
        assert!(store.has_key("anthropic"));
        assert_eq!(store.get("anthropic").as_deref(), Some("sk-first"));

        // Replace, not append.
        store.set("anthropic", "sk-second").unwrap();
        assert_eq!(store.get("anthropic").as_deref(), Some("sk-second"));

        // Empty and whitespace-only keys are rejected without storing.
        assert!(matches!(store.set("deepseek", ""), Err(AppError::Keys(_))));
        assert!(matches!(
            store.set("deepseek", "   "),
            Err(AppError::Keys(_))
        ));
        assert!(!store.has_key("deepseek"));

        // Remove, then the idempotent no-op leg.
        store.remove("anthropic").unwrap();
        assert!(!store.has_key("anthropic"));
        assert!(store.get("anthropic").is_none());
        store.remove("anthropic").unwrap();
    }

    #[test]
    fn dpapi_store_honours_the_keystore_contract() {
        let dir = tempfile::tempdir().unwrap();
        let store = DpapiKeyStore::load(dir.path().join("keys.json"));
        assert_keystore_contract(&store);
    }

    #[test]
    fn in_memory_store_honours_the_keystore_contract() {
        assert_keystore_contract(&InMemoryKeyStore::default());
    }

    #[test]
    fn missing_file_is_an_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let store = DpapiKeyStore::load(path.clone());
        assert!(!store.has_key("anthropic"));
        assert!(store.get("anthropic").is_none());
        assert!(!path.exists(), "load alone must not create the file");
    }

    #[test]
    fn set_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");

        let store = DpapiKeyStore::load(path.clone());
        store.set("anthropic", "sk-ant-123").unwrap();
        store.set("deepseek", "sk-ds-456").unwrap();

        // A fresh load decrypts both — the real DPAPI round trip.
        let reloaded = DpapiKeyStore::load(path);
        assert_eq!(reloaded.get("anthropic").as_deref(), Some("sk-ant-123"));
        assert_eq!(reloaded.get("deepseek").as_deref(), Some("sk-ds-456"));
    }

    #[test]
    fn set_replaces_an_existing_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");

        let store = DpapiKeyStore::load(path.clone());
        store.set("anthropic", "sk-old").unwrap();
        store.set("anthropic", "sk-new").unwrap();

        let reloaded = DpapiKeyStore::load(path);
        assert_eq!(reloaded.get("anthropic").as_deref(), Some("sk-new"));
    }

    #[test]
    fn remove_deletes_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");

        let store = DpapiKeyStore::load(path.clone());
        store.set("anthropic", "sk-gone").unwrap();
        store.remove("anthropic").unwrap();
        assert!(!store.has_key("anthropic"));

        let reloaded = DpapiKeyStore::load(path);
        assert!(!reloaded.has_key("anthropic"));
        assert!(reloaded.get("anthropic").is_none());
    }

    #[test]
    fn remove_absent_key_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let store = DpapiKeyStore::load(path.clone());
        store.remove("anthropic").unwrap();
        store.remove("anthropic").unwrap();
        assert!(!path.exists(), "a no-op remove must not create the file");
    }

    #[test]
    fn set_rejects_an_empty_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let store = DpapiKeyStore::load(path.clone());
        assert!(matches!(store.set("anthropic", ""), Err(AppError::Keys(_))));
        assert!(matches!(
            store.set("anthropic", "  \t "),
            Err(AppError::Keys(_))
        ));
        assert!(!path.exists(), "a rejected set must not create the file");
    }

    /// The no-cleartext pin (ADR): the pasted key must never appear in the
    /// raw file bytes.
    #[test]
    fn raw_file_never_contains_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");

        let store = DpapiKeyStore::load(path.clone());
        store.set("anthropic", SENTINEL).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        // Non-vacuous: the write really happened and holds the entry.
        assert!(raw.contains("anthropic"), "raw file was: {raw}");
        assert!(
            !raw.contains(SENTINEL),
            "cleartext key material landed on disk"
        );
    }

    /// The schema pin (ADR): version 1, a `keys` object, base64 values.
    #[test]
    fn file_schema_is_version_1_with_base64_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");

        let store = DpapiKeyStore::load(path.clone());
        store.set("anthropic", SENTINEL).unwrap();

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["version"], 1);
        let keys = saved["keys"].as_object().expect("keys is an object");
        assert!(!keys.is_empty());
        for (id, value) in keys {
            let blob = base64::engine::general_purpose::STANDARD
                .decode(value.as_str().expect("values are strings"))
                .unwrap_or_else(|e| panic!("value for {id} isn't base64: {e}"));
            assert!(!blob.is_empty());
            assert_ne!(blob, SENTINEL.as_bytes(), "value is just encoded cleartext");
        }
    }

    /// The ADR's migrated-machine rule: presence still reads true, `get`
    /// decays to None, and re-pasting over the ghost recovers.
    #[test]
    fn undecryptable_blob_is_treated_as_no_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let garbage = base64::engine::general_purpose::STANDARD.encode(b"not a dpapi blob");
        fs::write(
            &path,
            format!(r#"{{ "version": 1, "keys": {{ "anthropic": "{garbage}" }} }}"#),
        )
        .unwrap();

        let store = DpapiKeyStore::load(path);
        assert!(store.has_key("anthropic"), "presence never decrypts");
        assert!(store.get("anthropic").is_none());

        // Re-pasting over the ghost is the recovery path.
        store.set("anthropic", "sk-fresh").unwrap();
        assert_eq!(store.get("anthropic").as_deref(), Some("sk-fresh"));
    }

    #[test]
    fn undecodable_base64_is_treated_as_no_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        fs::write(
            &path,
            r#"{ "version": 1, "keys": { "anthropic": "not base64!!" } }"#,
        )
        .unwrap();

        let store = DpapiKeyStore::load(path);
        assert!(store.has_key("anthropic"));
        assert!(store.get("anthropic").is_none());
    }

    #[test]
    fn corrupt_file_is_backed_up_not_clobbered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        fs::write(&path, "definitely not json").unwrap();

        let store = DpapiKeyStore::load(path.clone());
        assert!(!store.has_key("anthropic"));

        // Original bytes survive in the .bak next to the store.
        let bak = fs::read_to_string(dir.path().join("keys.json.bak")).unwrap();
        assert_eq!(bak, "definitely not json");
        assert!(!path.exists(), "corrupt file should have been renamed away");
    }

    #[test]
    fn unreadable_file_refuses_mutations_instead_of_clobbering() {
        // A directory at the store path makes read_to_string fail with a
        // non-NotFound error — the portable stand-in for an AV/permission
        // lock. The store must refuse mutations, not persist its empty view.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        fs::create_dir(&path).unwrap();

        let store = DpapiKeyStore::load(path.clone());
        assert!(!store.has_key("anthropic"));
        assert!(matches!(
            store.set("anthropic", "sk-123"),
            Err(AppError::Keys(_))
        ));
        assert!(matches!(store.remove("anthropic"), Err(AppError::Keys(_))));
        assert!(path.is_dir(), "store path must not have been touched");
    }

    /// A valid file from a NEWER WikiLens: never renamed away, never
    /// rewritten as v1 — keys read as unset and mutations are refused.
    #[test]
    fn unknown_version_is_read_only_not_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let original = r#"{ "version": 2, "keys": { "anthropic": "ZnV0dXJl" } }"#;
        fs::write(&path, original).unwrap();

        let store = DpapiKeyStore::load(path.clone());
        assert!(!store.has_key("anthropic"), "v2 entries must not be exposed");
        assert!(matches!(
            store.set("anthropic", "sk-123"),
            Err(AppError::Keys(_))
        ));

        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert!(
            !dir.path().join("keys.json.bak").exists(),
            "a newer file is not corrupt — no .bak"
        );
    }

    #[test]
    fn persist_preserves_unknown_top_level_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        fs::write(
            &path,
            r#"{ "version": 1, "keys": {}, "future_field": { "rotation": "monthly" } }"#,
        )
        .unwrap();

        let store = DpapiKeyStore::load(path.clone());
        store.set("anthropic", "sk-123").unwrap();

        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["future_field"]["rotation"], "monthly");
        assert!(saved["keys"]["anthropic"].is_string());
    }
}
