---
title: WikiLens planning vault — index
type: note
status: active
created: 2026-07-02
updated: 2026-07-17
tags: []
related: []
---

# WikiLens planning vault

Plans, decisions, research, and retros for WikiLens. **Open this `vault/` folder as
the Obsidian vault** (not the repo root — that would index `node_modules`).

- **Naming:** `YYYY-MM-DD_<kebab-slug>.md` (creation date; treat as a permanent ID — don't rename).
- **Frontmatter:** typed properties on every doc — full reference in the table below.
- **New doc:** copy `templates/plan.md` (or `templates/decision.md`) and fill it in.
- **Vocabularies:** `type` ∈ {plan, decision, research, fix, retro, note};
  `status` ∈ {idea, todo, active, blocked, done, dropped}. Tags: use the registry below.
- **No secrets** in notes — this project holds provider API keys.

## Frontmatter fields

| field                 | value                                                                                                                                                                            |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `title`               | human sentence (also the `#` H1)                                                                                                                                                 |
| `type`                | plan · decision · research · fix · retro · note                                                                                                                                  |
| `status`              | idea · todo · active · blocked · done · dropped                                                                                                                                  |
| `created` / `updated` | bare ISO date `YYYY-MM-DD`; bump `updated` on every edit                                                                                                                         |
| `tags`                | topics from the registry below (closed set)                                                                                                                                      |
| `related`             | quoted wikilinks, e.g. `"[[2026-07-02_scaffold]]"`                                                                                                                               |
| `commit`              | the code this doc tracks — a single hash `85aca6c`, or a list `[a1b2c3d, e4f5g6h]` when it spans several commits. Optional. (Quote an all-digit hash so YAML keeps it a string.) |

> The dashboards below use [Dataview](https://github.com/blacksmithgu/obsidian-dataview).
> Without it they render as inert code blocks — the manual index and `grep` still work.

## Active board

```dataview
TABLE WITHOUT ID file.link AS Doc, type, status, updated
FROM !"templates" AND !"archive"
WHERE (status = "active" OR status = "blocked" OR status = "todo") AND file.name != "index"
SORT updated DESC
```

## Decision log

```dataview
TABLE WITHOUT ID file.link AS Decision, status, created
FROM !"templates" AND !"archive"
WHERE type = "decision"
SORT created DESC
```

## Recently touched (last 14 days)

```dataview
TABLE WITHOUT ID file.link AS Doc, type, status, updated
FROM !"templates" AND !"archive"
WHERE updated >= date(today) - dur(14 days) AND file.name != "index"
SORT updated DESC
```

---

## Index (manual fallback — grep `status:` if this drifts)

- **Active / blocked:** (none)
- **Todo:** [[2026-07-15_rewrite-bare-entity-candidate]] · [[2026-07-17_graphify-findings-review]] · [[2026-07-17_claude-md-capture-catchup]]
- **Todo (rewrite-review findings, in execution order):** [[2026-07-10_retrieval-path-timeouts]] · [[2026-07-10_rewrite-circuit-breaker]] · [[2026-07-10_reasoning-skip-and-capability-tags]] · [[2026-07-10_merge-raw-hit-guarantee]] · [[2026-07-10_rewrite-prompt-reword]] · [[2026-07-10_concurrent-candidate-searches-and-status]] · [[2026-07-10_retrieval-env-var-docs]]
- **Decisions:** [[2026-07-02_wikitext-extraction]] · [[2026-07-02_multi-provider-llm]] · [[2026-07-04_rendered-html-fetch-strategy]] · [[2026-07-05_model-list-sourcing]] · [[2026-07-07_llm-query-rewrite-in-retrieval]] · [[2026-07-13_pr-delivery-workflow]] · [[2026-07-16_codex-direct-claude-md-fallback]]
- **Done:** [[2026-07-02_scaffold]] · [[2026-07-02_multi-provider-llm]] · [[2026-07-02_wikitext-extraction]] · [[2026-07-03_commit-field-forms]] · [[2026-07-03_retrieval-integration-test]] · [[2026-07-04_search-srwhat-text]] · [[2026-07-04_query-preprocessing-zero-hit-retry]] · [[2026-07-04_golden-query-retrieval-tests]] · [[2026-07-04_rendered-html-extraction]] · [[2026-07-04_rendered-html-fetch-strategy]] · [[2026-07-05_model-list-sourcing]] · [[2026-07-05_model-picker-menu]] · [[2026-07-05_builtin-game-registry-expansion]] · [[2026-07-05_user-added-game-wikis]] · [[2026-07-05_game-picker-owned-menu]] · [[2026-07-05_design-tokens-claude-design-sync]] · [[2026-07-06_screenshot-capture-to-prompt]] · [[2026-07-06_model-vision-badges]] · [[2026-07-06_image-attach-guardrails]] · [[2026-07-07_retrieval-quality-improvements]] · [[2026-07-07_llm-query-rewrite-in-retrieval]] · [[2026-07-10_ask-debug-instrumentation]] · [[2026-07-13_testing-audit]] · [[2026-07-13_ci-pipeline-github-actions]] · [[2026-07-13_pr-delivery-workflow]] · [[2026-07-13_frontend-test-harness]] · [[2026-07-13_rust-http-mock-integration-tests]] · [[2026-07-13_parser-snapshot-property-tests]] · [[2026-07-13_dependency-audit-lint-gates]] · [[2026-07-13_manual-smoke-checklist-live-cadence]] · [[2026-07-13_wiki-fetch-hardening]] · [[2026-07-14_post-merge-dead-code-cleanup]] · [[2026-07-14_pr-description-anatomy]] · [[2026-07-15_cargo-manifest-eol-churn]] · [[2026-07-15_slow-wiki-status-hint]] · [[2026-07-16_wiki-403-friendly-error]] · [[2026-07-16_codex-config-mirror-skill]] · [[2026-07-16_codex-direct-claude-md-fallback]] · [[2026-07-16_model-menu-dropdown]]

Pure-grep, no Obsidian:
`grep -rl 'status: active' .` · `grep -rl 'type: decision' .`

## Tag registry (closed — add a tag here before using it)

`wiki` · `llm` · `rag` · `frontend` · `rust` · `tauri` · `overlay` · `hotkey` · `capture` · `security` · `build` · `vault` · `testing`

`vault` = the planning vault's own tooling and conventions. `capture` = in-game
screenshot capture and image attachments. `testing` = test infrastructure and
coverage work (harnesses, CI, guardrail suites). Reserved for when that work
starts: `cache`, `perf`.
