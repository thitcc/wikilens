---
title: Wiki-fetch security hardening (redirects, byte caps, prompt fencing)
type: plan
status: todo
created: 2026-07-13
updated: 2026-07-13
tags: [security, rust, wiki]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-05_user-added-game-wikis]]"]
commit:
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
