//! Persistent answer history.
//!
//! Backing file: `history.json` in the Tauri app-data dir (created in
//! `lib.rs`'s setup), a plain newest-first array of [`HistoryEntry`] capped at
//! [`HISTORY_CAP`]. Recording happens Rust-side at the `ask` success tail
//! (commands.rs) — best-effort, a failed write never fails the ask — and the
//! webview can only list/clear: there is no record command, so entries can't
//! be forged from the frontend. Same discipline as the other stores: every
//! mutation is persisted to disk *before* it becomes visible in memory.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::commands::Source;
use crate::error::AppError;

/// Newest-first cap: the append that would make it 51 drops the oldest.
pub const HISTORY_CAP: usize = 50;

/// One answered ask, as persisted and as sent to the frontend history menu
/// (field set pinned in `config_guardrails.rs`). Deliberately excluded: wiki
/// page text (the debug contract's rule — sources are title+url only), token
/// usage (dev-only; the debug window owns it), key material, and the
/// screenshot itself (`had_image` presence only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Store-minted `"{unix_millis}-{seq}"` — unique without a new dep.
    pub id: String,
    /// Wall-clock append time; the frontend renders it as relative time.
    pub created_ms: u64,
    pub game_id: String,
    /// Denormalized so a row still names its game after the game is removed.
    pub game_name: String,
    pub question: String,
    /// The full streamed answer, as markdown (LLM output — never wiki text).
    pub answer: String,
    pub sources: Vec<Source>,
    /// `targets.answer.name` — `"Local"` in Local mode.
    pub provider_name: String,
    /// The resolved model id in both modes. `Option` survives for the stored
    /// rows the removed Default mode wrote as `null` (its vendor id was
    /// deliberately dev-only); new entries always carry `Some`.
    pub model: Option<String>,
    pub had_image: bool,
}

/// What `ask` hands [`HistoryStore::append`]; `id` and `created_ms` are
/// minted by the store so the clock/id policy lives in one place.
pub struct NewEntry {
    pub game_id: String,
    pub game_name: String,
    pub question: String,
    pub answer: String,
    pub sources: Vec<Source>,
    pub provider_name: String,
    pub model: Option<String>,
    pub had_image: bool,
}

pub struct HistoryStore {
    path: PathBuf,
    entries: RwLock<Vec<HistoryEntry>>,
    /// Set when the file existed but couldn't be *read* at startup (AV lock,
    /// permissions, …). Mutations are refused then — persisting would replace
    /// the user's real file with this empty in-memory view.
    load_error: Option<String>,
    /// Tie-breaker for ids minted within the same millisecond.
    seq: AtomicU64,
}

impl HistoryStore {
    /// Load the store. A missing file is an empty history. A corrupt file is
    /// renamed sideways to `history.json.bak` — never silently overwritten —
    /// and the store starts empty. Any *other* read failure puts the store in
    /// a refuse-mutations state instead (same reasoning as `UserWikiStore`).
    pub fn load(path: PathBuf) -> Self {
        let mut load_error = None;
        let entries = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Vec<HistoryEntry>>(&raw) {
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
                    "wikilens: couldn't read {} ({e}); answer history is read-only this session",
                    path.display()
                );
                load_error = Some(e.to_string());
                Vec::new()
            }
        };
        Self {
            path,
            entries: RwLock::new(entries),
            load_error,
            seq: AtomicU64::new(0),
        }
    }

    /// Refuse mutations when the startup read failed (see `load_error`).
    fn guard_writable(&self) -> Result<(), AppError> {
        match &self.load_error {
            Some(e) => Err(AppError::History(format!(
                "your answer history couldn't be read when WikiLens started ({e}) — restart WikiLens and try again"
            ))),
            None => Ok(()),
        }
    }

    /// All entries, newest first (disk order IS newest first — appends
    /// front-insert). Clones out of the lock like the other stores.
    pub fn list(&self) -> Vec<HistoryEntry> {
        self.read().clone()
    }

    /// Record an answered ask: mints the id/timestamp, front-inserts, trims
    /// to [`HISTORY_CAP`], persists, and only then commits to memory.
    pub fn append(&self, new: NewEntry) -> Result<(), AppError> {
        self.guard_writable()?;
        let created_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let entry = HistoryEntry {
            id: format!("{created_ms}-{seq}"),
            created_ms,
            game_id: new.game_id,
            game_name: new.game_name,
            question: new.question,
            answer: new.answer,
            sources: new.sources,
            provider_name: new.provider_name,
            model: new.model,
            had_image: new.had_image,
        };
        let mut guard = self.write();
        let mut next = guard.clone();
        next.insert(0, entry);
        next.truncate(HISTORY_CAP);
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Wipe the history (the menu's pinned "Clear history" action). Also
    /// removes the sideways `history.json.bak` a corrupt-file load leaves
    /// behind — after a clear it would be the sole surviving copy of answers
    /// the player believes deleted.
    ///
    /// The `.bak` goes *first*: a clear must leave the store either fully
    /// cleared or fully intact. Removing it last meant a `.bak` another
    /// process held open (an editor, an AV scan) failed the command after the
    /// live file and the in-memory list were already empty — an error the
    /// frontend read as "nothing happened", keeping deleted rows selectable.
    pub fn clear(&self) -> Result<(), AppError> {
        self.guard_writable()?;
        let mut guard = self.write();
        match fs::remove_file(self.path.with_extension("json.bak")) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => {
                return Err(AppError::History(format!(
                    "its .bak copy couldn't be removed: {e} — nothing was cleared"
                )))
            }
        }
        let next = Vec::new();
        self.persist(&next)?;
        *guard = next;
        Ok(())
    }

    /// Crash-safe write: temp file in the same dir, then rename over the
    /// real one (std rename replaces existing files on Windows too).
    fn persist(&self, entries: &[HistoryEntry]) -> Result<(), AppError> {
        let storage = |e: &dyn std::fmt::Display| AppError::History(e.to_string());
        let dir = self
            .path
            .parent()
            .ok_or_else(|| AppError::History("no data directory".to_string()))?;
        fs::create_dir_all(dir).map_err(|e| storage(&e))?;
        let json = serde_json::to_string_pretty(entries).map_err(|e| storage(&e))?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| storage(&e))?;
        fs::rename(&tmp, &self.path).map_err(|e| storage(&e))
    }

    // Poisoning: same policy as the other stores — the data is a plain Vec,
    // safe to keep using after a panicked writer.
    fn read(&self) -> RwLockReadGuard<'_, Vec<HistoryEntry>> {
        self.entries.read().unwrap_or_else(PoisonError::into_inner)
    }
    fn write(&self) -> RwLockWriteGuard<'_, Vec<HistoryEntry>> {
        self.entries.write().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(question: &str) -> NewEntry {
        NewEntry {
            game_id: "stardew-valley".to_string(),
            game_name: "Stardew Valley".to_string(),
            question: question.to_string(),
            answer: format!("Answer to \"{question}\" in **markdown**."),
            sources: vec![Source {
                title: "Wood".to_string(),
                url: "https://stardewvalleywiki.com/Wood".to_string(),
            }],
            provider_name: "Anthropic".to_string(),
            model: Some("claude-sonnet-5".to_string()),
            had_image: false,
        }
    }

    #[test]
    fn missing_file_is_an_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::load(dir.path().join("history.json"));
        assert!(store.list().is_empty());
    }

    #[test]
    fn append_persists_and_reloads_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");

        let store = HistoryStore::load(path.clone());
        store.append(sample("first question")).unwrap();
        store.append(sample("second question")).unwrap();

        let reloaded = HistoryStore::load(path);
        let questions: Vec<String> = reloaded.list().into_iter().map(|e| e.question).collect();
        assert_eq!(questions, vec!["second question", "first question"]);
        // The entry round-trips whole: sources and metadata survive the disk.
        let entry = &reloaded.list()[0];
        assert_eq!(entry.game_name, "Stardew Valley");
        assert_eq!(entry.sources[0].url, "https://stardewvalleywiki.com/Wood");
        assert_eq!(entry.model.as_deref(), Some("claude-sonnet-5"));
    }

    #[test]
    fn cap_drops_the_oldest_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::load(dir.path().join("history.json"));
        for i in 0..HISTORY_CAP + 1 {
            store.append(sample(&format!("question {i}"))).unwrap();
        }
        let list = store.list();
        assert_eq!(list.len(), HISTORY_CAP);
        // Newest survives at the front; "question 0" fell off the end.
        assert_eq!(list[0].question, format!("question {HISTORY_CAP}"));
        assert_eq!(list[HISTORY_CAP - 1].question, "question 1");
    }

    #[test]
    fn clear_empties_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");

        let store = HistoryStore::load(path.clone());
        store.append(sample("a question")).unwrap();
        store.clear().unwrap();
        assert!(store.list().is_empty());

        let reloaded = HistoryStore::load(path);
        assert!(reloaded.list().is_empty());
    }

    #[test]
    fn unreadable_file_refuses_mutations_instead_of_clobbering() {
        // A directory at the store path makes read_to_string fail with a
        // non-NotFound error — the portable stand-in for an AV/permission
        // lock. The store must refuse mutations, not persist its empty view.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::create_dir(&path).unwrap();

        let store = HistoryStore::load(path.clone());
        assert!(store.list().is_empty());
        assert!(matches!(
            store.append(sample("a question")),
            Err(AppError::History(_))
        ));
        assert!(matches!(store.clear(), Err(AppError::History(_))));
        assert!(path.is_dir(), "store path must not have been touched");
    }

    #[test]
    fn corrupt_file_is_backed_up_not_clobbered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(&path, "definitely not json").unwrap();

        let store = HistoryStore::load(path.clone());
        assert!(store.list().is_empty());

        let bak = fs::read_to_string(dir.path().join("history.json.bak")).unwrap();
        assert_eq!(bak, "definitely not json");
        assert!(!path.exists(), "corrupt file should have been renamed away");
    }

    #[test]
    fn clear_removes_the_sideways_backup_too() {
        // After "Clear history" the .bak from a corrupt-file load must not
        // outlive the answers the player just deleted (security review
        // 2026-08-15, item S6).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let bak = dir.path().join("history.json.bak");
        fs::write(&path, "definitely not json").unwrap();

        let store = HistoryStore::load(path.clone());
        assert!(bak.exists(), "precondition: the corrupt file was kept as .bak");
        store.append(sample("a question")).unwrap();
        store.clear().unwrap();

        assert!(!bak.exists(), "clear() must remove history.json.bak");
        assert!(HistoryStore::load(path).list().is_empty());
        // Clearing again, with no .bak around, is still a clean success.
        store.clear().unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn clear_leaves_everything_intact_when_the_bak_is_locked() {
        // A .bak another process holds open without FILE_SHARE_DELETE (an
        // editor, an AV scan) can't be removed. The clear must then fail
        // *before* touching the live file — a half-cleared store made the
        // menu keep offering rows Rust had already deleted.
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let bak = dir.path().join("history.json.bak");
        fs::write(&path, "definitely not json").unwrap();

        let store = HistoryStore::load(path.clone());
        assert!(bak.exists(), "precondition: the corrupt file was kept as .bak");
        store.append(sample("a question")).unwrap();

        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&bak)
            .unwrap();
        let err = store.clear().unwrap_err().to_string();
        assert!(
            err.contains("nothing was cleared"),
            "the error must say the history is intact: {err}"
        );
        assert_eq!(store.list().len(), 1, "in-memory list must be untouched");
        let on_disk: Vec<HistoryEntry> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(on_disk.len(), 1, "live file must be untouched");
        assert!(bak.exists());

        // Once the other process lets go, the same clear succeeds whole.
        drop(lock);
        store.clear().unwrap();
        assert!(store.list().is_empty());
        assert!(!bak.exists());
        assert!(HistoryStore::load(path).list().is_empty());
    }

    #[test]
    fn rapid_appends_mint_unique_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::load(dir.path().join("history.json"));
        for i in 0..10 {
            store.append(sample(&format!("q{i}"))).unwrap();
        }
        let mut ids: Vec<String> = store.list().into_iter().map(|e| e.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10, "same-millisecond appends must not collide");
    }
}
