---
title: Default mode as a subscription — provisioned per-user keys
type: plan
status: idea
created: 2026-08-04
updated: 2026-08-04
tags: [llm, security]
related:
  - "[[2026-07-26_default-mode-and-byo-api-keys]]"
  - "[[2026-07-26_api-key-storage-dpapi]]"
  - "[[2026-07-29_keys-are-not-a-mode-choice]]"
commit:
---

# Default mode as a subscription — provisioned per-user keys

## Context / problem

A packaged install has no `WIKILENS_DEFAULT_*` env, so Default mode is dead
on arrival for a stranger; only BYO-key Custom mode works out of the box.
The monetization question: could Default mode become "$X/month and it just
works"? Bundling the app-owner's vendor key in the executable is ruled out
hard — anything shipped is extractable (strings, memory dump, a local proxy
reading the auth header), unrevocable, and unbounded spend on the owner's
account. Whatever ships must put only a *bounded, revocable* secret on the
user's machine. Visual explainer: `docs/default-mode-subscription.html`.

## Goal / non-goals

- Goal: a paid Default mode with per-user quota enforcement, instant
  revocation, and no master key on any user machine.
- Non-goals: building payment/billing UI into the overlay; a full streaming
  proxy on day one (a later migration path, not the first move); changing
  free BYO-key Custom mode, which stays as-is.

## Approach

Sketch, OpenRouter-provisioning-keys-first (verified against their docs
2026-08-04):

1. Tiny cold-path backend (webhook-scale): payment webhook mints a runtime
   key via `POST /api/v1/keys/` with `limit` = monthly quota and
   `limit_reset: monthly`; cancellation is `PATCH {disabled: true}`.
2. The minted key reaches the user and lives in the existing DPAPI store;
   asks go straight to OpenRouter — the hot path never touches the backend.
3. App-side, "subscriber mode" is Default mode pointed at OpenRouter:
   `providers.rs` already speaks its protocol, `llm.rs` already streams its
   SSE, `target.rs` already treats the endpoint as config. The one *new*
   app piece: a Default resolver that reads the subscriber key from the
   DPAPI store instead of env (today `resolve_default_targets` is
   env-only), gated so the pasted key still never flips the mode by
   itself, per [[2026-07-29_keys-are-not-a-mode-choice]].
4. Blast radius of an extracted key = that user's own monthly quota, so
   extraction is annoying rather than catastrophic.
5. If per-request control, model hiding, or margin ever matter: migrate the
   hot path behind an owned proxy speaking the same wire protocol; the
   `target.rs` seam makes that an endpoint swap.

## Decisions & trade-offs

Costs accepted in the sketch: ~5.5% credit-purchase fee and list-price
inference (price the subscription above expected usage), no per-key model
allowlist (the credit cap bounds abuse), calendar-month resets at midnight
UTC rather than signup anniversaries.

## Status log

- 2026-08-04 — created as an idea after v0.1.0's first packaged install
  surfaced the "Default mode needs env" gap; explainer page written the
  same day.
