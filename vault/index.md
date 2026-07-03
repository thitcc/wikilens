---
title: WikiLens planning vault — index
type: note
status: active
created: 2026-07-02
updated: 2026-07-03
tags: []
related: []
---

# WikiLens planning vault

Plans, decisions, research, and retros for WikiLens. **Open this `vault/` folder as
the Obsidian vault** (not the repo root — that would index `node_modules`).

- **Naming:** `YYYY-MM-DD_<kebab-slug>.md` (creation date; treat as a permanent ID — don't rename).
- **Frontmatter:** `title, type, status, created, updated, tags, related, commit`.
- **New doc:** copy `templates/plan.md` (or `templates/decision.md`) and fill it in.
- **Vocabularies:** `type` ∈ {plan, decision, research, fix, retro, note};
  `status` ∈ {idea, todo, active, blocked, done, dropped}. Tags: use the registry below.
- **No secrets** in notes — this project holds provider API keys.

> The dashboards below use [Dataview](https://github.com/blacksmithgu/obsidian-dataview).
> Without it they render as inert code blocks — the manual index and `grep` still work.

## Active board

```dataview
TABLE WITHOUT ID file.link AS Doc, type, status, updated
FROM !"templates" AND !"archive"
WHERE (status = "active" OR status = "blocked") AND file.name != "index"
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
- **Decisions:** [[2026-07-02_wikitext-extraction]] · [[2026-07-02_multi-provider-llm]]
- **Done:** [[2026-07-02_scaffold]] · [[2026-07-02_multi-provider-llm]] · [[2026-07-02_wikitext-extraction]]

Pure-grep, no Obsidian:
`grep -rl 'status: active' .` · `grep -rl 'type: decision' .`

## Tag registry (closed — add a tag here before using it)

`wiki` · `llm` · `rag` · `frontend` · `rust` · `tauri` · `overlay` · `hotkey` · `security` · `build`

Reserved for when that work starts: `cache`, `perf`.
