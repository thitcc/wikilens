//! Opt-in per-ask debug table, gated by the `WIKILENS_DEBUG` env var.
//!
//! `run_ask` fills a [`DebugReport`] as it goes; the report prints itself to
//! stderr **on drop**, so every exit path — success, the soft-fail early
//! returns, and `?` error propagation — emits the (possibly partial) table.
//! Collection is unconditional (a few `Instant`s and small clones per ask);
//! only the printing is gated, so call sites carry no `if debug` branches.
//!
//! The table shows phase timings, models, token counts, queries, and page
//! titles with char counts — **never** the wiki text sent to the model and
//! never API keys. Pre-flight failures (empty question, unknown game/provider,
//! missing key) happen before the report exists and print no table.
//!
//! Independent of `WIKILENS_TRACE_RETRIEVAL`, whose one-line records are
//! consumed by the offline eval tooling and stay byte-identical.
//!
//! When the visual debug window exists (the flag was truthy at startup), the
//! report additionally emits `debug://…` events through an injected
//! [`DebugSink`] as it is filled — same payload contract as the table: titles,
//! counts, queries, and timings only; **never** wiki text, never keys. The
//! sink keeps this module tauri-free: `debug_window::sink` supplies the
//! `emit_to` closure, and tests supply a recording one.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::llm::TokenUsage;

/// Table width for the rule lines; wide enough for a long OpenRouter model id
/// without wrapping a typical terminal.
const RULE_WIDTH: usize = 76;

/// Monotonic id source so the debug window can group events per ask (the
/// `capture::NEXT_ID` pattern). Claimed in [`DebugReport::new`].
static NEXT_ASK_ID: AtomicU64 = AtomicU64::new(1);

/// Where `debug://…` events go. Injected (not constructed here) so this module
/// stays tauri-free and unit tests can record emissions with a plain closure.
pub type DebugSink = Box<dyn Fn(&str, serde_json::Value) + Send + Sync>;

/// Header info at ask start — the table's header rows, minus nothing.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AskStartedPayload<'a> {
    ask_id: u64,
    game: &'a str,
    question: &'a str,
    query: &'a str,
    answer_model: &'a str,
    /// `null` when the rewrite pair matches the answer pair (same rule as the
    /// table's optional rewrite row); always present so the key set is fixed.
    rewrite_model: Option<&'a str>,
}

/// One phase row; `elapsed_ms: null` = the phase was skipped, not 0 ms.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhasePayload<'a> {
    ask_id: u64,
    name: &'a str,
    elapsed_ms: Option<u64>,
    detail: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidatesPayload<'a> {
    ask_id: u64,
    candidates: &'a [String],
}

/// Token usage for one call. A missing field mirrors the table's `?` (the
/// call happened, the provider reported nothing); a never-made call simply
/// never emits this event (the `-/-` case).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UsagePayload {
    ask_id: u64,
    kind: &'static str,
    input: Option<u64>,
    output: Option<u64>,
}

/// Page title + extracted-plaintext char count — the text itself never
/// reaches this module, so it structurally cannot cross IPC here.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PageEntry<'a> {
    title: &'a str,
    chars: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PagesPayload<'a> {
    ask_id: u64,
    pages: Vec<PageEntry<'a>>,
}

/// Emitted from `Drop` on every exit path — the window's equivalent of the
/// partial table. `aborted` = no deliberate exit was recorded.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FinishedPayload<'a> {
    ask_id: u64,
    total_ms: u64,
    outcome: &'a str,
    aborted: bool,
}

/// One row of the phase table. `elapsed: None` renders as `-` (the phase was
/// skipped, e.g. rewrite disabled) as opposed to a measured `0`.
struct PhaseRow {
    name: String,
    elapsed: Option<Duration>,
    detail: String,
}

/// Per-ask debug collector; see the module doc for the print-on-drop contract.
pub struct DebugReport {
    enabled: bool,
    ask_id: u64,
    /// `debug://…` event outlet; `None` when no debug window exists. Emission
    /// is additionally gated on `enabled`, like the table print.
    sink: Option<DebugSink>,
    started: Instant,
    game: String,
    question: String,
    query: String,
    answer_model: String,
    /// `Some` only when the rewrite provider/model differs from the answer pair.
    rewrite_model: Option<String>,
    candidates: Vec<String>,
    phases: Vec<PhaseRow>,
    /// `None` = the call was never made (`-/-`); `Some` but empty = the call was
    /// made and the provider reported nothing (`?/?`).
    rewrite_usage: Option<TokenUsage>,
    answer_usage: Option<TokenUsage>,
    /// Page title + char count of the extracted plaintext (count only — the
    /// text itself never reaches this module).
    pages: Vec<(String, usize)>,
    /// Set at the deliberate exits; `None` at drop time means the ask aborted
    /// (an error propagated or the future was cancelled).
    outcome: Option<&'static str>,
}

impl DebugReport {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        game_name: &str,
        game_id: &str,
        question: &str,
        query: &str,
        answer_provider: &str,
        answer_model: &str,
        rewrite_provider: &str,
        rewrite_model: &str,
    ) -> Self {
        let answer_pair = format!("{answer_provider} / {answer_model}");
        let rewrite_pair = format!("{rewrite_provider} / {rewrite_model}");
        DebugReport {
            enabled: debug_enabled(),
            ask_id: NEXT_ASK_ID.fetch_add(1, Ordering::Relaxed),
            sink: None,
            started: Instant::now(),
            game: format!("{game_name} ({game_id})"),
            question: question.to_string(),
            query: query.to_string(),
            rewrite_model: (rewrite_pair != answer_pair).then_some(rewrite_pair),
            answer_model: answer_pair,
            candidates: Vec::new(),
            phases: Vec::new(),
            rewrite_usage: None,
            answer_usage: None,
            pages: Vec::new(),
            outcome: None,
        }
    }

    /// Attach the `debug://…` outlet and immediately announce the ask (the
    /// header rows). Called once, right after `new` — every later setter then
    /// mirrors itself through the sink.
    pub fn attach_sink(&mut self, sink: DebugSink) {
        self.sink = Some(sink);
        self.emit(
            "debug://ask-started",
            &AskStartedPayload {
                ask_id: self.ask_id,
                game: &self.game,
                question: &self.question,
                query: &self.query,
                answer_model: &self.answer_model,
                rewrite_model: self.rewrite_model.as_deref(),
            },
        );
    }

    /// Send one event through the sink. A no-op unless enabled and attached,
    /// so call sites stay unconditional (same discipline as the table print).
    fn emit(&self, event: &str, payload: &impl Serialize) {
        if !self.enabled {
            return;
        }
        if let (Some(sink), Ok(value)) = (&self.sink, serde_json::to_value(payload)) {
            sink(event, value);
        }
    }

    /// Test-only deterministic override of the env-derived flag — the
    /// multithreaded suite must never mutate the process environment.
    #[cfg(test)]
    pub(crate) fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Record a timed phase row, in execution order.
    pub fn phase(&mut self, name: &str, elapsed: Duration, detail: String) {
        self.emit(
            "debug://phase",
            &PhasePayload {
                ask_id: self.ask_id,
                name,
                elapsed_ms: Some(elapsed.as_millis() as u64),
                detail: &detail,
            },
        );
        self.phases.push(PhaseRow {
            name: name.to_string(),
            elapsed: Some(elapsed),
            detail,
        });
    }

    /// Record a phase that never ran (renders `-` in the ms column).
    pub fn phase_skipped(&mut self, name: &str, detail: &str) {
        self.emit(
            "debug://phase",
            &PhasePayload {
                ask_id: self.ask_id,
                name,
                elapsed_ms: None,
                detail,
            },
        );
        self.phases.push(PhaseRow {
            name: name.to_string(),
            elapsed: None,
            detail: detail.to_string(),
        });
    }

    pub fn set_candidates(&mut self, candidates: &[String]) {
        self.emit(
            "debug://candidates",
            &CandidatesPayload {
                ask_id: self.ask_id,
                candidates,
            },
        );
        self.candidates = candidates.to_vec();
    }

    /// Only call when the rewrite request was actually sent — `None` vs
    /// `Some(empty)` is the `-/-` vs `?/?` distinction in the tokens row.
    pub fn set_rewrite_usage(&mut self, usage: TokenUsage) {
        self.emit_usage("rewrite", &usage);
        self.rewrite_usage = Some(usage);
    }

    /// Only call when the answer request was actually sent (see above).
    pub fn set_answer_usage(&mut self, usage: TokenUsage) {
        self.emit_usage("answer", &usage);
        self.answer_usage = Some(usage);
    }

    fn emit_usage(&self, kind: &'static str, usage: &TokenUsage) {
        self.emit(
            "debug://usage",
            &UsagePayload {
                ask_id: self.ask_id,
                kind,
                input: usage.input,
                output: usage.output,
            },
        );
    }

    pub fn set_pages(&mut self, pages: Vec<(String, usize)>) {
        self.emit(
            "debug://pages",
            &PagesPayload {
                ask_id: self.ask_id,
                pages: pages
                    .iter()
                    .map(|(title, chars)| PageEntry {
                        title,
                        chars: *chars,
                    })
                    .collect(),
            },
        );
        self.pages = pages;
    }

    /// Mark a deliberate exit; unset at drop time renders as aborted.
    pub fn finish(&mut self, outcome: &'static str) {
        self.outcome = Some(outcome);
    }

    /// Pure renderer — `total` is passed in (the `Drop` impl supplies the real
    /// elapsed time) so tests can assert exact output.
    fn render(&self, total: Duration) -> String {
        let mut out = String::new();
        let rule = "-".repeat(RULE_WIDTH);
        let title = "wikilens.debug ";
        out.push_str(title);
        out.push_str(&"-".repeat(RULE_WIDTH.saturating_sub(title.len())));
        out.push('\n');

        let mut row = |label: &str, value: String| {
            out.push_str(&format!("  {label:<10} {value}\n"));
        };
        row("game", one_line(&self.game));
        row("question", one_line(&self.question));
        row("query", one_line(&self.query));
        row("model", one_line(&self.answer_model));
        if let Some(rewrite) = &self.rewrite_model {
            row("rewrite", one_line(rewrite));
        }
        if !self.candidates.is_empty() {
            let list = self
                .candidates
                .iter()
                .map(|c| format!("\"{}\"", one_line(c)))
                .collect::<Vec<_>>()
                .join(" | ");
            row("candidates", list);
        }

        out.push_str(&rule);
        out.push('\n');
        out.push_str(&format!("  {:<18}{:>7}  {}\n", "phase", "ms", "detail"));
        for phase in &self.phases {
            let ms = match phase.elapsed {
                Some(elapsed) => elapsed.as_millis().to_string(),
                None => "-".to_string(),
            };
            out.push_str(&format!(
                "  {:<18}{ms:>7}  {}\n",
                phase.name,
                one_line(&phase.detail)
            ));
        }

        out.push_str(&rule);
        out.push('\n');
        out.push_str(&format!(
            "  {:<10} rewrite in/out {} | answer in/out {}\n",
            "tokens",
            usage_pair(&self.rewrite_usage),
            usage_pair(&self.answer_usage)
        ));
        if !self.pages.is_empty() {
            let list = self
                .pages
                .iter()
                .map(|(title, chars)| format!("{} ({chars})", one_line(title)))
                .collect::<Vec<_>>()
                .join(" | ");
            out.push_str(&format!("  {:<10} {list}\n", "pages"));
        }
        out.push_str(&format!(
            "  {:<10} {} ms   outcome: {}\n",
            "total",
            total.as_millis(),
            self.outcome.unwrap_or(ABORTED_OUTCOME)
        ));
        out.push_str(&rule);
        out
    }
}

impl Drop for DebugReport {
    fn drop(&mut self) {
        let total = self.started.elapsed();
        // `emit_to` is synchronous, so signalling from Drop is safe — this is
        // what lets errors, soft-fails, and cancelled futures reach the debug
        // window, mirroring the partial table below.
        self.emit(
            "debug://finished",
            &FinishedPayload {
                ask_id: self.ask_id,
                total_ms: total.as_millis() as u64,
                outcome: self.outcome.unwrap_or(ABORTED_OUTCOME),
                aborted: self.outcome.is_none(),
            },
        );
        if self.enabled {
            // One eprintln for the whole table so concurrent trace lines can't
            // interleave into the middle of it.
            eprintln!("{}", self.render(total));
        }
    }
}

/// The outcome text shared by the table's total row and the `finished`
/// payload when no deliberate exit was recorded before drop.
const ABORTED_OUTCOME: &str = "aborted (error or cancelled)";

/// The `WIKILENS_DEBUG` gate, read per use so it stays a plain function of the
/// environment. Also consulted at startup (`lib.rs`, `tray.rs`) to decide
/// whether the debug window and its tray item exist at all — env can't change
/// mid-process, so that startup decision always agrees with the per-ask read.
pub(crate) fn debug_enabled() -> bool {
    std::env::var("WIKILENS_DEBUG")
        .map(|v| is_truthy(&v))
        .unwrap_or(false)
}

/// Opt-IN flag check: set and not a falsey word. Deliberately not
/// `commands::stage_enabled`, whose default-when-unset is ON — an opt-in flag
/// must default OFF.
pub(crate) fn is_truthy(value: &str) -> bool {
    !value.is_empty() && !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "off" | "no")
}

/// `-/-` when the call was never made; `?` per missing field when the provider
/// reported nothing for a call that did happen.
fn usage_pair(usage: &Option<TokenUsage>) -> String {
    match usage {
        None => "-/-".to_string(),
        Some(u) => format!("{}/{}", opt_count(u.input), opt_count(u.output)),
    }
}

fn opt_count(n: Option<u64>) -> String {
    n.map_or_else(|| "?".to_string(), |v| v.to_string())
}

/// Questions and queries are user text and may hold newlines; the table is
/// line-oriented, so flatten them.
fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truthy_matrix() {
        for on in ["1", "true", "TRUE", "yes", "on", "anything"] {
            assert!(is_truthy(on), "{on} should enable");
        }
        for off in ["", "0", "false", "FALSE", "off", "OFF", "no", "No"] {
            assert!(!is_truthy(off), "{off:?} should disable");
        }
    }

    /// A fully-populated report renders the exact documented layout — this is
    /// the golden test that locks column alignment.
    #[test]
    fn renders_full_report() {
        let mut report = DebugReport::new(
            "Stardew Valley",
            "stardew-valley",
            "best crops for winter?",
            "best crops winter",
            "deepseek",
            "deepseek-chat",
            "anthropic",
            "claude-haiku-4-5-20251001",
        );
        report.phase("rewrite", Duration::from_millis(412), "2 candidates".into());
        report.phase("raw search", Duration::from_millis(380), "5 hits".into());
        report.phase("cand search", Duration::from_millis(690), "2 queries -> 7 hits".into());
        report.phase("fetch", Duration::from_millis(1893), "2 pages, 17121 chars".into());
        report.phase("answer", Duration::from_millis(6120), "first token 940 ms".into());
        report.set_candidates(&["Winter crops".to_string(), "Crop profitability".to_string()]);
        report.set_rewrite_usage(TokenUsage {
            input: Some(184),
            output: Some(22),
        });
        report.set_answer_usage(TokenUsage {
            input: Some(15890),
            output: Some(312),
        });
        report.set_pages(vec![("Winter".to_string(), 9120), ("Crops".to_string(), 8001)]);
        report.finish("answered");

        // Rules are derived from RULE_WIDTH (not hand-counted); every content
        // row is literal — this is what locks the column alignment.
        let rule = "-".repeat(RULE_WIDTH);
        let top = "-".repeat(RULE_WIDTH - "wikilens.debug ".len());
        let expected = format!(
            "\
wikilens.debug {top}
  game       Stardew Valley (stardew-valley)
  question   best crops for winter?
  query      best crops winter
  model      deepseek / deepseek-chat
  rewrite    anthropic / claude-haiku-4-5-20251001
  candidates \"Winter crops\" | \"Crop profitability\"
{rule}
  phase                  ms  detail
  rewrite               412  2 candidates
  raw search            380  5 hits
  cand search           690  2 queries -> 7 hits
  fetch                1893  2 pages, 17121 chars
  answer               6120  first token 940 ms
{rule}
  tokens     rewrite in/out 184/22 | answer in/out 15890/312
  pages      Winter (9120) | Crops (8001)
  total      9495 ms   outcome: answered
{rule}"
        );
        assert_eq!(report.render(Duration::from_millis(9495)), expected);
    }

    #[test]
    fn renders_degraded_report() {
        // Rewrite skipped, same rewrite model (no header row), usage absent on
        // the answer call, no pages, no outcome (aborted).
        let mut report = DebugReport::new(
            "Hollow Knight",
            "hollow-knight",
            "line one\nline two",
            "wanderers jornal",
            "anthropic",
            "claude-haiku-4-5-20251001",
            "anthropic",
            "claude-haiku-4-5-20251001",
        );
        report.phase_skipped("rewrite", "disabled (WIKILENS_QUERY_REWRITE)");
        report.phase("raw search", Duration::from_millis(420), "0 hits".into());
        report.set_answer_usage(TokenUsage::default());

        let rendered = report.render(Duration::from_millis(500));
        // Multi-line question flattened.
        assert!(rendered.contains("  question   line one line two\n"));
        // No rewrite header row (the model pair appears only in the model row)
        // and no candidates row.
        assert_eq!(
            rendered.matches("anthropic / claude-haiku-4-5-20251001").count(),
            1
        );
        assert!(!rendered.contains("candidates"));
        // Skipped phase renders `-` in the ms column, right-aligned.
        assert!(rendered.contains("  rewrite                 -  disabled (WIKILENS_QUERY_REWRITE)\n"));
        // -/- = never called; ?/? = called, nothing reported.
        assert!(rendered.contains("rewrite in/out -/- | answer in/out ?/?"));
        // No pages row.
        assert!(!rendered.contains("\n  pages"));
        // Unset outcome renders as aborted.
        assert!(rendered.contains("outcome: aborted (error or cancelled)"));
    }

    #[test]
    fn empty_report_does_not_panic() {
        let report = DebugReport::new("g", "id", "q?", "q", "p", "m", "p", "m");
        let rendered = report.render(Duration::ZERO);
        assert!(rendered.contains("outcome: aborted"));
        assert!(rendered.contains("  total      0 ms"));
    }

    // ---- debug:// sink emission ----

    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex};

    use serde_json::Value;

    type Recorded = Arc<Mutex<Vec<(String, Value)>>>;

    /// A plain recording closure — the whole point of the sink design is that
    /// tests never need a tauri runtime to observe emissions.
    fn recording_sink() -> (DebugSink, Recorded) {
        let events: Recorded = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&events);
        let sink: DebugSink = Box::new(move |event, payload| {
            record.lock().unwrap().push((event.to_string(), payload));
        });
        (sink, events)
    }

    fn sample_report() -> DebugReport {
        DebugReport::new(
            "Stardew Valley",
            "stardew-valley",
            "best crops for winter?",
            "best crops winter",
            "deepseek",
            "deepseek-chat",
            "anthropic",
            "claude-haiku-4-5-20251001",
        )
    }

    fn keys(value: &Value) -> BTreeSet<String> {
        value
            .as_object()
            .expect("payload serializes to an object")
            .keys()
            .cloned()
            .collect()
    }

    fn set<const N: usize>(names: [&str; N]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    /// The full event stream in order, with the exact serialized key set of
    /// every payload pinned — the structural guarantee that no field carrying
    /// wiki text or key material can be added without updating this test.
    #[test]
    fn sink_receives_ordered_events_with_pinned_field_sets() {
        let (sink, events) = recording_sink();
        let mut report = sample_report().with_enabled(true);
        report.attach_sink(sink);
        report.phase_skipped("rewrite", "disabled (WIKILENS_QUERY_REWRITE)");
        report.phase("raw search", Duration::from_millis(380), "5 hits".into());
        report.set_candidates(&["Winter crops".to_string()]);
        report.set_rewrite_usage(TokenUsage {
            input: Some(184),
            output: Some(22),
        });
        report.set_answer_usage(TokenUsage::default());
        report.set_pages(vec![("Winter".to_string(), 9120)]);
        report.finish("answered");
        drop(report);

        let events = events.lock().unwrap();
        let names: Vec<&str> = events.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "debug://ask-started",
                "debug://phase",
                "debug://phase",
                "debug://candidates",
                "debug://usage",
                "debug://usage",
                "debug://pages",
                "debug://finished",
            ]
        );

        let started = &events[0].1;
        assert_eq!(
            keys(started),
            set(["askId", "game", "question", "query", "answerModel", "rewriteModel"])
        );
        assert_eq!(started["game"], "Stardew Valley (stardew-valley)");
        // Rewrite pair differs from the answer pair here, so the field is set.
        assert_eq!(started["rewriteModel"], "anthropic / claude-haiku-4-5-20251001");

        assert_eq!(keys(&events[1].1), set(["askId", "name", "elapsedMs", "detail"]));
        assert!(events[1].1["elapsedMs"].is_null(), "skipped phase carries null");
        assert_eq!(events[2].1["elapsedMs"], 380);

        assert_eq!(keys(&events[3].1), set(["askId", "candidates"]));

        assert_eq!(keys(&events[4].1), set(["askId", "kind", "input", "output"]));
        assert_eq!(events[4].1["kind"], "rewrite");
        assert_eq!(events[5].1["kind"], "answer");
        // The `?/?` case: the call happened, the provider reported nothing.
        assert!(events[5].1["input"].is_null());

        assert_eq!(keys(&events[6].1), set(["askId", "pages"]));
        for page in events[6].1["pages"].as_array().expect("pages array") {
            assert_eq!(keys(page), set(["title", "chars"]));
        }

        let finished = &events[7].1;
        assert_eq!(keys(finished), set(["askId", "totalMs", "outcome", "aborted"]));
        assert_eq!(finished["outcome"], "answered");
        assert_eq!(finished["aborted"], false);

        // Every payload carries the same ask id.
        let id = started["askId"].as_u64().expect("numeric askId");
        assert!(events.iter().all(|(_, p)| p["askId"] == id));
    }

    /// Dropping without `finish` — an error `?` or a cancelled future — still
    /// emits `finished`, flagged aborted: the window's partial-table analogue.
    #[test]
    fn drop_without_finish_signals_aborted() {
        let (sink, events) = recording_sink();
        let mut report = sample_report().with_enabled(true);
        report.attach_sink(sink);
        drop(report);

        let events = events.lock().unwrap();
        let (name, payload) = events.last().expect("finished emitted");
        assert_eq!(name, "debug://finished");
        assert_eq!(payload["aborted"], true);
        assert_eq!(payload["outcome"], ABORTED_OUTCOME);
    }

    #[test]
    fn disabled_report_emits_nothing() {
        let (sink, events) = recording_sink();
        let mut report = sample_report().with_enabled(false);
        report.attach_sink(sink);
        report.phase("raw search", Duration::from_millis(10), "3 hits".into());
        report.finish("answered");
        drop(report);
        assert!(events.lock().unwrap().is_empty());
    }

    /// No sink attached (no debug window) must stay harmless on every path —
    /// enabled here, so drop also walks the emit + table-print branches.
    #[test]
    fn enabled_report_without_sink_does_not_panic() {
        let mut report = sample_report().with_enabled(true);
        report.phase("raw search", Duration::from_millis(1), "1 hits".into());
        report.finish("answered");
    }

    #[test]
    fn ask_ids_are_distinct_and_rewrite_model_null_when_pair_matches() {
        let (sink_a, events_a) = recording_sink();
        let mut first = sample_report().with_enabled(true);
        first.attach_sink(sink_a);

        // Same rewrite/answer pair → rewriteModel serializes as null.
        let (sink_b, events_b) = recording_sink();
        let mut second = DebugReport::new("g", "id", "q?", "q", "p", "m", "p", "m").with_enabled(true);
        second.attach_sink(sink_b);

        let id_a = events_a.lock().unwrap()[0].1["askId"].as_u64().unwrap();
        let started_b = events_b.lock().unwrap()[0].1.clone();
        assert!(started_b["askId"].as_u64().unwrap() > id_a, "ids are monotonic");
        assert!(started_b["rewriteModel"].is_null());
    }
}
