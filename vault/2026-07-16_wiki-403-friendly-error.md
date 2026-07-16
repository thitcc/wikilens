---
title: Show a friendly message when a wiki blocks WikiLens with a 403
type: plan
status: done
created: 2026-07-16
updated: 2026-07-16
tags: [wiki, rust]
related:
  - "[[2026-07-15_slow-wiki-status-hint]]"
  - "[[2026-07-13_wiki-fetch-hardening]]"
commit: 7f9aa1b
---

# Show a friendly message when a wiki blocks WikiLens with a 403

## Context / problem

The Guild Wars 2 wiki (wiki.guildwars2.com, behind an AWS ELB + WAF) now returns
403 Forbidden on **every** `api.php` and `rest.php` request from a non-browser
client. Verified 2026-07-16: the block ignores User-Agent (WikiLens's UA, a full
Chrome header set, bare curl all get 403), holds from two unrelated networks, and
covers even parameterless `api.php` — while regular `/wiki/` pages,
`index.php?action=raw`, and the HTML `Special:Search` all still return 200. The
403 is served by `awselb/2.0` before MediaWiki ever sees the request, so it's
almost certainly TLS-fingerprint/path-based anti-AI-scraper protection (public
forum threads show the wiki fighting crawler traffic with firewall rules through
early 2026).

For the player, an ask against GW2 dies at the first search and the error box
shows the raw reqwest string:
`Network request failed: HTTP status client error (403 Forbidden) for url (https://wiki.guildwars2.com/api.php?…)`.
That reads like a WikiLens bug; it's the wiki refusing automated access.

## Goal / non-goals

- Goal: when a wiki answers 403, the error box says in plain language that the
  wiki blocks automated access — one mapping that covers search, both page-fetch
  paths, and the title index, for built-ins and user-added wikis alike.
- Non-goal: 429/rate-limit or other status-specific copy.
- Non-goal: an HTML-scrape fallback for blocked wikis (`Special:Search` + `/wiki/`
  pages still work, but routing around a deliberate block deserves its own
  decision doc on etiquette before anyone builds it).
- Non-goal: removing GW2 from the built-ins — the block may be lifted, and asking
  ArenaNet to whitelist a light interactive client (via the wiki's "Reporting
  wiki bugs" page) remains an open option.

## Approach

Single-point mapping in `error.rs`, mirroring the existing `error_chain(.0)`
helper-in-format-attribute pattern: the `Http` variant's message becomes
`#[error("{}", http_message(.0))]`, where `http_message` returns the friendly
copy when `err.status() == Some(403)` and the byte-identical
`Network request failed: {error_chain}` otherwise. Copy:

> This wiki refused WikiLens's request (403 Forbidden) — it looks like it blocks
> automated access. Its pages still open normally in a browser.

Status-bearing `Http` errors only come from `error_for_status()` on the wiki
paths (search.rs, fetch.rs ×2, titles.rs) — llm.rs builds `AppError::Llm` itself,
models.rs degrades to curated fallbacks, probe.rs maps to `AppError::Probe` — so
"This wiki" is accurate everywhere the message can surface. No frontend change:
App.tsx renders Rust error strings verbatim.

Tests (wiremock — reqwest has no public constructor for a status-carrying error):
a Display pin in `http.rs::http_tests` (403 → friendly copy), a search-path test
in `search.rs::http_tests`, a fetch-ladder test in `fetch.rs::http_tests` (403 on
parse **and** revisions), and a guard extending
`fallback_error_surfaces_when_nothing_fetched` so a 500 keeps the generic
message.

## Decisions & trade-offs

- **Single generic message over a per-call-site named-wiki variant.** A
  `WikiBlocked { wiki }` variant would name the wiki in the copy, but needs a
  helper remembered at every current and future wiki call site; the Display-level
  mapping covers all four sites (and future ones) in one arm, and the error box
  already sits under the game chip that names the game. Chosen with the user.
- The GW2 golden-case live test (`cargo test golden -- --ignored`) fails while
  the WAF block stands — expected external breakage, not a regression.

## Status log

- 2026-07-16 — created; GW2 block diagnosed and approach agreed.
- 2026-07-16 — done: `http_message` Display arm + wiremock pins (http/search/
  fetch) landed in 7f9aa1b; `/check` green. Remaining smoke: ask a GW2 question
  in the dev app and see the friendly copy (the live block is the fixture).
