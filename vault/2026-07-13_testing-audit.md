---
title: Testing audit — coverage inventory, verified gaps, and adopt/skip verdicts
type: research
status: done
created: 2026-07-13
updated: 2026-07-13
tags: [testing, rust, frontend, security]
related: ["[[2026-07-13_ci-pipeline-github-actions]]", "[[2026-07-13_frontend-test-harness]]", "[[2026-07-13_rust-http-mock-integration-tests]]", "[[2026-07-13_parser-snapshot-property-tests]]", "[[2026-07-13_dependency-audit-lint-gates]]", "[[2026-07-13_manual-smoke-checklist-live-cadence]]", "[[2026-07-13_wiki-fetch-hardening]]"]
commit:
---

# Testing audit — coverage inventory, verified gaps, and adopt/skip verdicts

## In simple terms
The codebase is entirely AI-generated, so we audited whether the tests actually
prove it works and is safe. Independent reviewers inventoried every existing test
(and ran the suite), hunted for risky untested code, tried to *disprove* each
other's findings, and researched what good testing looks like for a Rust + Tauri
app in 2026. Verdict: the test base is genuinely solid; the weaknesses are
structural — no CI, no frontend tests, network code untested offline — not
security holes.

## Method
Multi-agent audit run 2026-07-12: four parallel surveyors (Rust test inventory,
security-gap hunt, frontend assessment, best-practices research with sources),
one adversarial verifier per claimed gap, one completeness critic. All 14 claimed
gaps were confirmed real as *coverage gaps*; all 14 had their *risk* downgraded
to low once verified against the actual code and threat model (single-user
Windows gaming desktop, strict CSP, no webview network permissions).

## Coverage inventory (at `df0bfda`)

| Layer | State |
|---|---|
| Rust pure logic (parsers, registries, probe derivation) | **Strong** — 138 behavioral tests, 128 offline-green in ~3s, realistic inline fixtures, registry-invariant guardrails |
| Rust network & orchestration (`answer_streaming` loop, `fetch_pages` ladder, `run_ask`, `probe_base` flow, `fetch_all_titles`) | **Untested offline** — no mock-HTTP dev-dep exists; `commands.rs` (~750 lines, the core) has 6 tests of two tiny helpers |
| Live integration (19-case golden-query suite, probes, OpenRouter catalog) | **Good pattern, no cadence** — 10 `#[ignore]`-gated tests, run only ad hoc |
| Frontend (React/TS) | **Zero** — no tests, no tooling; only gate is `tsc` inside `npm run build` |
| Runtime-only surface (overlay, hotkey, tray, DPI) | Untestable by automation; no written manual checklist either |
| Process (CI, lint gates, dependency audit) | **Absent** — no `.github/workflows`, no clippy gate, no cargo-audit/npm audit |

## Verified security findings (all low risk — reasoning recorded so severity stays honest)
- **SSRF via `add_game` → `probe_base`:** `normalize_base_url` accepts
  localhost/private IPs; the shared client follows up to 10 redirects unguarded
  (`state.rs`). Low because the webview has no network perms + strict CSP (thin
  attack vector), there is no cloud metadata service on a home desktop, and
  localhost wikis are a legitimate use case. → fix plan: [[2026-07-13_wiki-fetch-hardening]]
- **No byte caps on wiki fetches:** every path does uncapped `.text().await`; a
  hostile wiki could OOM the app. Low: requires the user to add a hostile wiki,
  payoff is only DoS-ing their own overlay. → same fix plan.
- **Prompt injection from wiki content:** excerpts spliced unfenced into the LLM
  prompt (`llm.rs`). Better guarded than it looks: exact message bytes and system
  prompt are already regression-tested; answers render as markdown with no raw
  HTML, links click-gated to the system browser, keys never enter the prompt —
  zero-click exfiltration channels are closed.
- **API keys:** verified never logged, never cross IPC. Missing only a guardrail
  test pinning this. → [[2026-07-13_dependency-audit-lint-gates]]

## Adopt — in priority order
1. [[2026-07-13_ci-pipeline-github-actions]] — the 138 tests exist and nothing runs them.
2. [[2026-07-13_frontend-test-harness]] — the only layer with literally zero coverage.
3. [[2026-07-13_rust-http-mock-integration-tests]] — the biggest untested Rust surface.
4. [[2026-07-13_parser-snapshot-property-tests]] — locks in the hard-won extraction behaviors.
5. [[2026-07-13_dependency-audit-lint-gates]] — cheap; turns CLAUDE.md prose into asserts.
6. [[2026-07-13_manual-smoke-checklist-live-cadence]] — the compensating control for the runtime-only surface.

Side item (fixes, not tests): [[2026-07-13_wiki-fetch-hardening]].

## Defer
- `tauri::test::mock_builder` for IPC-boundary command tests — official but API
  still flagged unstable; revisit for `add_game`/`remove_game`.
- cargo-nextest + cargo-llvm-cov — nice runner/coverage diagnostics, not needed
  to start; tarpaulin explicitly avoided (Linux-centric).

## Skip — decided, don't re-litigate
- **WebDriver E2E** (tauri-driver / WebdriverIO): viable on Windows now, but it
  cannot reach the riskiest surfaces (global hotkey, overlay placement, DPI) and
  adds an msedgedriver version-matching tax a solo app doesn't repay.
- **Coverage-percentage gates:** coverage stays an occasional diagnostic, never CI policy.
- **Mutation testing (cargo-mutants)** and **cargo-fuzz:** overkill at this size;
  proptest no-panic properties capture ~90% of the fuzzing value.
- **Tray/hotkey plumbing unit tests:** requires a Tauri runtime; covered by the
  manual checklist instead.
- **CSS/geometry assertions and whole-tree React snapshots:** validated by eye;
  classic over-testing for a glass overlay.

## Sources (research agent, mid-2026)
- https://v2.tauri.app/develop/tests/ (+ /mocking/, /webdriver/)
- https://github.com/LukeMathWalker/wiremock-rs · https://github.com/httpmock/httpmock
- https://github.com/mitsuhiko/insta · https://rust-fuzz.github.io/book/cargo-fuzz.html
- https://nexte.st/docs/integrations/test-coverage/ · https://github.com/taiki-e/cargo-llvm-cov
- https://vitest.dev/ · https://webdriver.io/docs/desktop-testing/tauri/platform-support/
- https://v2.tauri.app/distribute/pipelines/github/ · https://github.com/tauri-apps/tauri-action

## Status log
- 2026-07-13 — created; audit ran 2026-07-12 in-session (19 agents, adversarial
  verify pass). Recommendations split into the seven linked plan docs, all
  `status: todo` — deliberately no implementation yet.
