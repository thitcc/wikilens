//! Persistent store for user-added game wikis.
//!
//! Backing file: `wikis.json` in the Tauri app-data dir (created in
//! `lib.rs`'s setup), a plain array of `GameWiki` objects. Loaded once at
//! startup and managed as Tauri state; every mutation is persisted to disk
//! *before* it becomes visible in memory, so the two can't drift apart.
//! Entries only land here after probe validation (`wiki/probe.rs`) — the
//! registry stays Rust-side and `ask` never fetches a URL the frontend
//! supplies per-request.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::error::AppError;
use crate::wiki::games::GameWiki;

pub struct UserWikiStore {
    path: PathBuf,
    wikis: RwLock<Vec<GameWiki>>,
    /// Set when the file existed but couldn't be *read* at startup (AV lock,
    /// permissions, …). Mutations are refused then — persisting would replace
    /// the user's real file with this empty in-memory view.
    load_error: Option<String>,
}

impl UserWikiStore {
    /// Load the store. A missing file is an empty store. A corrupt file is
    /// renamed sideways to `wikis.json.bak` — never silently overwritten —
    /// and the store starts empty. Any *other* read failure puts the store in
    /// a refuse-mutations state instead: on Windows a transient AV/backup
    /// lock is realistic, and treating it as "empty" would let the next add
    /// clobber the file.
    pub fn load(path: PathBuf) -> Self {
        let mut load_error = None;
        let wikis = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Vec<GameWiki>>(&raw) {
                Ok(list) => list,
                Err(e) => {
                    eprintln!(
                        "wikilens: {} is corrupt ({e}); keeping it as .bak and starting empty",
                        path.display()
                    );
                    let _ = fs::rename(&path, path.with_extension("json.bak"));
                    Vec::new()
                }
            },
            Err(e) if e.kind() == ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                eprintln!(
                    "wikilens: couldn't read {} ({e}); user games are read-only this session",
                    path.display()
                );
                load_error = Some(e.to_string());
                Vec::new()
            }
        };
        Self {
            path,
            wikis: RwLock::new(wikis),
            load_error,
        }
    }

    /// Refuse mutations when the startup read failed (see `load_error`).
    fn guard_writable(&self) -> Result<(), AppError> {
        match &self.load_error {
            Some(e) => Err(AppError::Storage(format!(
                "your saved games couldn't be read when WikiLens started ({e}) — restart WikiLens and try again"
            ))),
            None => Ok(()),
        }
    }

    /// All user wikis, sorted by display name for the picker.
    pub fn list(&self) -> Vec<GameWiki> {
        let mut list = self.read().clone();
        list.sort_by_key(|w| w.name.to_lowercase());
        list
    }

    /// Clone one entry out of the lock (callers hold it across awaits).
    pub fn get(&self, id: &str) -> Option<GameWiki> {
        self.read().iter().find(|w| w.id == id).cloned()
    }

    /// Add a wiki: persisted first, committed to memory only on success.
    /// Duplicate ids are rejected (built-in collisions are the command
    /// layer's check — this store doesn't know about the built-in registry).
    pub fn add(&self, wiki: GameWiki) -> Result<(), AppError> {
        self.guard_writable()?;
        let mut guard = self.write();
        if guard.iter().any(|w| w.id == wiki.id) {
            return Err(AppError::InvalidGame(format!(
                "\"{}\" is already in your games — remove it first, or use a different name.",
                wiki.name
            )));
        }
        let mut next = guard.clone();
        next.push(wiki);
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Remove a user wiki by id. Built-in ids never live in this store, so
    /// they can't be removed here by construction.
    pub fn remove(&self, id: &str) -> Result<(), AppError> {
        self.guard_writable()?;
        let mut guard = self.write();
        if !guard.iter().any(|w| w.id == id) {
            return Err(AppError::UnknownGame(id.to_string()));
        }
        let next: Vec<GameWiki> = guard.iter().filter(|w| w.id != id).cloned().collect();
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Crash-safe write: temp file in the same dir, then rename over the
    /// real one (std rename replaces existing files on Windows too).
    fn persist(&self, wikis: &[GameWiki]) -> Result<(), AppError> {
        let storage = |e: &dyn std::fmt::Display| AppError::Storage(e.to_string());
        let dir = self
            .path
            .parent()
            .ok_or_else(|| AppError::Storage("no data directory".to_string()))?;
        fs::create_dir_all(dir).map_err(|e| storage(&e))?;
        let json = serde_json::to_string_pretty(wikis).map_err(|e| storage(&e))?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| storage(&e))?;
        fs::rename(&tmp, &self.path).map_err(|e| storage(&e))
    }

    // Poisoning: same policy as AppState::models_cache — the data is a plain
    // Vec, safe to keep using after a panicked writer.
    fn read(&self) -> RwLockReadGuard<'_, Vec<GameWiki>> {
        self.wikis.read().unwrap_or_else(PoisonError::into_inner)
    }
    fn write(&self) -> RwLockWriteGuard<'_, Vec<GameWiki>> {
        self.wikis.write().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, name: &str) -> GameWiki {
        GameWiki {
            id: id.to_string(),
            name: name.to_string(),
            api_url: format!("https://{id}.example/api.php"),
            page_url: format!("https://{id}.example/wiki/"),
            search_namespace: None,
        }
    }

    #[test]
    fn missing_file_is_an_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = UserWikiStore::load(dir.path().join("wikis.json"));
        assert!(store.list().is_empty());
        assert!(store.get("anything").is_none());
    }

    #[test]
    fn add_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wikis.json");

        let store = UserWikiStore::load(path.clone());
        store.add(sample("palworld", "Palworld")).unwrap();
        store.add(sample("valheim", "Valheim")).unwrap();

        // A fresh load sees exactly what was added.
        let reloaded = UserWikiStore::load(path);
        assert_eq!(
            reloaded.get("palworld").map(|w| w.name),
            Some("Palworld".to_string())
        );
        assert_eq!(reloaded.list().len(), 2);
    }

    #[test]
    fn list_is_sorted_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = UserWikiStore::load(dir.path().join("wikis.json"));
        store.add(sample("zzz", "Zelda-like")).unwrap();
        store.add(sample("aaa", "another game")).unwrap();
        let names: Vec<String> = store.list().into_iter().map(|w| w.name).collect();
        // Case-insensitive: "another" before "Zelda".
        assert_eq!(names, vec!["another game", "Zelda-like"]);
    }

    #[test]
    fn duplicate_id_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = UserWikiStore::load(dir.path().join("wikis.json"));
        store.add(sample("palworld", "Palworld")).unwrap();
        let err = store.add(sample("palworld", "Palworld")).unwrap_err();
        assert!(matches!(err, AppError::InvalidGame(_)));
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn remove_deletes_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wikis.json");

        let store = UserWikiStore::load(path.clone());
        store.add(sample("palworld", "Palworld")).unwrap();
        store.remove("palworld").unwrap();
        assert!(store.get("palworld").is_none());

        // Removing an unknown id is an error, not a no-op.
        assert!(matches!(
            store.remove("palworld"),
            Err(AppError::UnknownGame(_))
        ));

        let reloaded = UserWikiStore::load(path);
        assert!(reloaded.list().is_empty());
    }

    #[test]
    fn unreadable_file_refuses_mutations_instead_of_clobbering() {
        // A directory at the store path makes read_to_string fail with a
        // non-NotFound error — the portable stand-in for an AV/permission
        // lock. The store must refuse mutations, not persist its empty view.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wikis.json");
        fs::create_dir(&path).unwrap();

        let store = UserWikiStore::load(path.clone());
        assert!(store.list().is_empty());
        assert!(matches!(
            store.add(sample("palworld", "Palworld")),
            Err(AppError::Storage(_))
        ));
        assert!(matches!(store.remove("palworld"), Err(AppError::Storage(_))));
        assert!(path.is_dir(), "store path must not have been touched");
    }

    #[test]
    fn corrupt_file_is_backed_up_not_clobbered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wikis.json");
        fs::write(&path, "definitely not json").unwrap();

        let store = UserWikiStore::load(path.clone());
        assert!(store.list().is_empty());

        // Original bytes survive in the .bak next to the store.
        let bak = fs::read_to_string(dir.path().join("wikis.json.bak")).unwrap();
        assert_eq!(bak, "definitely not json");
        assert!(!path.exists(), "corrupt file should have been renamed away");
    }
}
