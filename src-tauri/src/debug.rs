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

use std::time::{Duration, Instant};

use crate::llm::TokenUsage;

/// Table width for the rule lines; wide enough for a long OpenRouter model id
/// without wrapping a typical terminal.
const RULE_WIDTH: usize = 76;

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
            enabled: std::env::var("WIKILENS_DEBUG")
                .map(|v| is_truthy(&v))
                .unwrap_or(false),
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

    /// Record a timed phase row, in execution order.
    pub fn phase(&mut self, name: &str, elapsed: Duration, detail: String) {
        self.phases.push(PhaseRow {
            name: name.to_string(),
            elapsed: Some(elapsed),
            detail,
        });
    }

    /// Record a phase that never ran (renders `-` in the ms column).
    pub fn phase_skipped(&mut self, name: &str, detail: &str) {
        self.phases.push(PhaseRow {
            name: name.to_string(),
            elapsed: None,
            detail: detail.to_string(),
        });
    }

    pub fn set_candidates(&mut self, candidates: &[String]) {
        self.candidates = candidates.to_vec();
    }

    /// Only call when the rewrite request was actually sent — `None` vs
    /// `Some(empty)` is the `-/-` vs `?/?` distinction in the tokens row.
    pub fn set_rewrite_usage(&mut self, usage: TokenUsage) {
        self.rewrite_usage = Some(usage);
    }

    /// Only call when the answer request was actually sent (see above).
    pub fn set_answer_usage(&mut self, usage: TokenUsage) {
        self.answer_usage = Some(usage);
    }

    pub fn set_pages(&mut self, pages: Vec<(String, usize)>) {
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
            self.outcome.unwrap_or("aborted (error or cancelled)")
        ));
        out.push_str(&rule);
        out
    }
}

impl Drop for DebugReport {
    fn drop(&mut self) {
        if self.enabled {
            // One eprintln for the whole table so concurrent trace lines can't
            // interleave into the middle of it.
            eprintln!("{}", self.render(self.started.elapsed()));
        }
    }
}

/// Opt-IN flag check: set and not a falsey word. Deliberately not
/// `commands::stage_enabled`, whose default-when-unset is ON — a debug flag
/// must default OFF.
fn is_truthy(value: &str) -> bool {
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
}
