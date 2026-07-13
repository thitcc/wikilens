---
title: Wiki-fetch security hardening (redirects, byte caps, prompt fencing)
type: plan
status: done
created: 2026-07-13
updated: 2026-07-13
tags: [security, rust, wiki]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-05_user-added-game-wikis]]", "[[2026-07-13_rust-http-mock-integration-tests]]"]
commit: [6022b9a, a1db9d7, 768ae32]
---

# Wiki-fetch security hardening (redirects, byte caps, prompt fencing)

## In simple terms
Three belt-and-suspenders fixes from the security half of the testing audit.
None is an open hole — the audit's adversarial verifiers rated all three low
risk for this app — but each is cheap to close and cheap to keep closed with a
pinning test. These are code changes, not tests; the audit's side item.

## Context / problem
From [[2026-07-13_testing-audit]], with the verified-low reasoning recorded so
the priority stays honest:
- The shared reqwest client (`state.rs`) sets only a User-Agent, so reqwest's
  default follow-10-redirects policy is active — a wiki added via `add_game`
  ([[2026-07-05_user-added-game-wikis]]) can redirect probes anywhere. Low risk:
  webview has no network perms + strict CSP, no cloud metadata endpoint on a
  gaming desktop, and localhost/LAN wikis are legitimate.
- Every wiki fetch does uncapped `.text().await` (`fetch.rs`, `search.rs`,
  `titles.rs`, `probe.rs`) and the HTML reducer runs on the full body before
  truncation — a hostile or broken wiki can OOM/hang the app. Low risk: the user
  must add the hostile wiki themselves; payoff is DoS-ing their own overlay.
- Wiki excerpts are spliced unfenced into the LLM prompt (`llm.rs`
  `build_user_message`). Low risk: message framing is already pinned by tests,
  answers are display-only markdown, keys never enter the prompt.

## Goal / non-goals
- **Goal:** shrink the reachable surface without breaking legitimate use —
  localhost and LAN wikis must keep working.
- **Non-goal:** a blanket private-IP blocklist (would break the LAN-wiki use
  case; the decision inside this plan is whether refusing only *cross-host
  redirects* gets the benefit without the cost).
- **Non-goal:** sanitizing wiki text for instruction-like content (unwinnable;
  fencing + the existing display-only rendering is the defense).

## Approach
1. **Redirect policy** on the shared client: `redirect::Policy::custom` — keep a
   small hop limit, refuse scheme downgrades and (pending the decision above)
   cross-host hops from user-added wikis. Pin with a unit test on the client
   builder plus a wiremock redirect test once
   [[2026-07-13_rust-http-mock-integration-tests]] lands.
2. **Byte caps:** check `Content-Length` when present and stream-with-cap
   otherwise (`bytes_stream` + running total) before handing to parsers; pick a
   generous cap (wiki pages are KBs; the OpenRouter catalog ~1-2 MB is the
   ceiling case). Give the two currently *untimed* requests (batched revisions
   fallback, search) the same timeout the others have.
3. **Prompt fencing (optional, last):** wrap each excerpt in a clearly-delimited
   untrusted-data envelope; update the existing exact-bytes message tests — they
   currently pin the *unfenced* format and will guide the change.

## Decisions & trade-offs
- Each change ships with the test that pins it — this plan follows, not
  precedes, the audit's testing priorities on purpose: hardening without a
  regression net just erodes.
- Caps and policies live in one place (`state.rs` client construction + a
  constants block) so future fetch paths inherit them.

## Status log
- 2026-07-13 — created from [[2026-07-13_testing-audit]]; side item alongside
  priorities 1-6. No work started.
- 2026-07-13 — **done** in three commits. Notes against the plan as written:
  - **Home deviation:** policy lives in a new `src-tauri/src/http.rs`, not
    `state.rs` — same "one place" intent, but the four wiremock tiers and
    seven live-suite builders all import the factory, and
    `crate::http::build_client()` is the cleaner path (`AppState::new` is now
    a one-liner). Every client construction in the tree goes through it, so
    the tests exercise the shipped policy (`6022b9a`).
  - **The open cross-host decision resolved: uniform policy, cross-host
    allowed.** Hop limit 5 + refuse https→http downgrades. Wiki origin is
    erased by fetch time (threading a user-added flag wasn't worth it),
    apex→www and wiki-farm moves are legitimate, and LAN/localhost targets
    are supported anyway — cross-host refusal bought nothing. The downgrade
    rule is the load-bearing part: reqwest does not strip Anthropic's
    `x-api-key` on redirects, and refusing the downgrade keeps a key off
    plaintext HTTP. The deliberate cross-host allow is itself pinned by a
    two-server wiremock test so a future tightening is visible.
  - Two behavior changes flagged for review: client-builder failure now
    panics at startup (`expect`) instead of silently degrading to an
    unhardened `Client::new()`; and `AppError::Http` Display walks the error
    source chain (reqwest 0.12 stopped inlining sources — without this the
    policy's refusal reason never reached the user).
  - **Caps** (`a1db9d7`): 8 MiB `MAX_RESPONSE_BYTES` on every whole-body read
    via `http::read_body_capped` (counts *stream* bytes — gzip loses
    Content-Length and yields decompressed bytes; a flate2-built gzip-bomb
    test pins it), 16 KiB truncate-don't-fail `read_error_body` for non-2xx
    bodies (previously unbounded into the error box), 1 MiB
    `MAX_STREAM_BYTES` running total on the SSE answer loop (also bounds the
    line buffer against a newline-less flood). New `AppError::BodyTooLarge`.
  - **Timeouts**: search 8s (hottest request; was untimed), revisions
    fallback reuses the 12s `PARSE_TIMEOUT` (no duplicate const), rewrite 15s
    — `run_ask` joins the rewrite with the retry search, so a stalled rewrite
    provider used to wedge every zero-hit ask forever; errors already degrade
    to "no candidates". Client-wide: 10s connect + 60s read-gap (kills
    mid-stream stalls without bounding healthy streams); the answer stream
    deliberately keeps no total timeout. The two slow timeout pins live in
    the `--ignored` tier (they take the full 8s/12s to pass).
  - **Fencing** (`768ae32`): per-page `<wiki_excerpt title="…">` tags (one
    shared builder covers both protocols and text/image shapes) + the
    system-prompt untrusted-data rule. NOT a sanitizer — an embedded fake
    closing tag passes through verbatim, pinned by a test so nobody "fixes"
    it later. Bite-check found a real masking bug in my first wire pin: the
    bare `<wiki_excerpt` also appears in the system prompt, so the matchers
    pin the `<wiki_excerpt title=` form only the user message emits.
  - **Live validation:** full `--ignored` run after the caps commit — 11/12
    green through the new client (all wiki round-trips, probes, OpenRouter
    catalog). The golden suite passed 8 straight cases (incl. conanexiles,
    Steel Bar #1) then hit wiki.guildwars2.com 403s — an IP-wide CDN
    cool-down from today's unusually many suite runs, not the client change
    (identical 403 via PowerShell's HTTP stack with both wikilens and
    browser UAs, and the same wiki 403'd transiently this morning *before*
    these changes). Once the cool-down lifted (~25 min, confirmed by a
    once-a-minute probe) the golden rerun came back **20/20 green under the
    new client** — 19 OK incl. gw2's Mesmer, 1 known-gap (Abigail-gift; the
    fusion-core known-gap actually hit this run), 0 strict misses. Net: the
    whole registry is verified live under the new policy, and the cadence
    doc's "rerun once" guidance gains a corollary — repeated same-day suite
    runs can extend a CDN cool-down from seconds to ~half an hour.
