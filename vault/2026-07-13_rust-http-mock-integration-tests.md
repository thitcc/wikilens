---
title: Offline HTTP-mock integration tests for streaming and fetch orchestration
type: plan
status: todo
created: 2026-07-13
updated: 2026-07-13
tags: [testing, rust, llm, wiki]
related: ["[[2026-07-13_testing-audit]]", "[[2026-07-03_retrieval-integration-test]]"]
commit:
---

# Offline HTTP-mock integration tests for streaming and fetch orchestration

## In simple terms
The parsers are well tested, but the loops that pull bytes off the network and
feed those parsers are not — and they can't be, offline, because the real wikis
and LLM providers are currently the only servers the code can talk to. A fake
local server (wiremock) lets tests replay any response — a mid-stream provider
error, a page that fails to render, a wrong auth header — without the network.
Priority 3 of the testing audit.

## Context / problem
No mock-HTTP dev-dependency exists; every network round-trip is untested offline
([[2026-07-13_testing-audit]]): the `llm::answer_streaming` byte-buffered SSE
loop, `fetch::fetch_pages`' rendered-HTML→wikitext fallback ladder, all request
builders (auth headers never asserted), `models::fetch_models`, and
`probe_base`/`suggest` control flow. The live `#[ignore]` tier
([[2026-07-03_retrieval-integration-test]]) proves the wire contract but can't
exercise failure shapes on demand.

## Goal / non-goals
- **Goal:** deterministic offline tests for the network/orchestration layer,
  driven by a local wiremock server.
- **Non-goal:** `tauri::test::mock_builder` IPC-boundary tests — deferred (API
  flagged unstable); revisit for `add_game`/`remove_game`.
- **Non-goal:** chunk-*timing* tests (dripped SSE) — mock servers deliver bodies
  whole; an axum escape hatch exists if ever truly needed.

## Approach
- Add `wiremock` as a dev-dependency.
- **Prerequisite refactor:** make base URLs injectable so tests can point code at
  `MockServer::uri()` — wiki endpoints are already data (`GameWiki.api_url`);
  LLM/model endpoints live in the `Provider` registry and need a test override.
- Targets, in value order:
  1. `answer_streaming`: mid-stream `{"error":...}` on an HTTP-200 stream aborts
     with `AppError::Llm{status:200}`; deltas split across chunk boundaries
     (`set_body_raw` SSE body — reqwest still consumes it incrementally);
     stream-close-without-`[DONE]` (Anthropic) returns accumulated text; non-2xx
     routing (VisionUnsupported vs the Llm backstop).
  2. `fetch_pages`: per-title parse failure falls back to the single batched
     revisions request; fallback error surfaces only when nothing else fetched;
     empty-render skip.
  3. Request builders: `x-api-key` + `anthropic-version` vs `Bearer`, stream
     flag, `max_tokens` — asserted via wiremock matchers on the received request.
  4. `models::fetch_models`: keyless path, non-2xx → `AppError::Llm`.
  5. `probe_base`/`suggest`: candidate iteration order on siteinfo failure;
     "siteinfo answered, so a search failure is a verdict, not a retry".

## Decisions & trade-offs
- **wiremock over httpmock/mockito:** async-first (matches tokio/reqwest),
  background server pool keeps parallel tests fast, extensible matchers;
  community default. httpmock is the fallback if record/playback is ever wanted.
- Injectable URLs add a constructor parameter or two — accepted; it's the one
  structural change this plan is allowed to make.

## Status log
- 2026-07-13 — created from [[2026-07-13_testing-audit]]; queued as priority 3.
  No work started.
