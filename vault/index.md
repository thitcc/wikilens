---
title: WikiLens planning vault — index
type: note
status: active
created: 2026-07-02
updated: 2026-07-19
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

> The dashboards below use [Dataview](https://github.com/blacksmithgu/obsidian-dataview),
> which derives them from frontmatter — there is no hand-maintained doc list to keep in
> sync. Without the plugin they render as inert code blocks; `grep` still works.

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

## Without Obsidian

Frontmatter is the source of truth; these read it directly:
`grep -rl 'status: active' .` · `grep -rl 'type: decision' .`

## Tag registry (closed — add a tag here before using it)

`wiki` · `llm` · `rag` · `frontend` · `rust` · `tauri` · `overlay` · `hotkey` · `capture` · `security` · `build` · `vault` · `testing`

`vault` = the planning vault's own tooling and conventions. `capture` = in-game
screenshot capture and image attachments. `testing` = test infrastructure and
coverage work (harnesses, CI, guardrail suites). Reserved for when that work
starts: `cache`, `perf`.
