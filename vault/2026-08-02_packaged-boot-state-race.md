---
title: Packaged app failed at boot — webview raced setup's store manage() calls
type: fix
status: done
created: 2026-08-02
updated: 2026-08-02
tags: [tauri, rust]
related: ["[[2026-07-13_dependency-audit-lint-gates]]", "[[2026-07-13_manual-smoke-checklist-live-cadence]]"]
commit: c59b403
---

# Packaged app failed at boot — webview raced setup's store manage() calls

## Context / problem
The first packaged build (v0.1.0 exe) showed a red error box on launch:
"state not managed for field `keys` on command `list_providers`. You must
call `.manage()` before using this command" — with the header stuck on
"Loading games…" (`list_games` needs `UserWikiStore`, failing the same way).
`npm run tauri dev` had never shown it once.

## Root cause
Tauri creates `create: true` config windows **before** the `.setup()` hook
runs (verified in tauri 2.11.5 `app.rs`: the window loop sits immediately
above the setup call), so the overlay webview boots in parallel with our
setup closure, and its first invokes race the `.manage()` calls. The stores
were managed at the **end** of setup — after tray creation, hotkey
registration, and (with `WIKILENS_DEBUG` set, as on the reporting machine)
the creation of a second WebView2 window, hundreds of ms of gap. Dev never
lost the race because the page loads from the Vite server; the packaged
build serves bundled assets near-instantly and its boot invokes
(`list_games`, `list_providers`) landed mid-setup. `get_settings` survived
because `SettingsStore` was managed by setup's fifth statement — which is also
why the race stayed invisible until the store manage() calls grew work in
front of them.

## Fix
Invert the order instead of shrinking the gap: both config windows carry
`"create": false` and are built in setup via
`WebviewWindowBuilder::from_config` **after** all three stores are managed
(the sanctioned pattern — tauri-utils documents `create: false` with exactly
this from_config-in-setup example). No webview exists before its state does,
so the race is gone by construction, not by timing. The
`disable_os_open_transition` call folds into the same creation loop; tray,
hotkey, and debug window follow. Guardrail test
(`config_windows_defer_creation_to_setup`) pins `"create": false` on every
configured window; CLAUDE.md §5 gotcha + a smoke-checklist item 10 box cover
the humans.

## Verification
- `/check` gates green (clippy, cargo test 291 incl. the new guardrail,
  Vitest 155, test:node 38, tsc).
- Rebuilt packaged exe survives boot with `WIKILENS_DEBUG=1` (10s, stderr
  silent — a setup failure aborts the process), and the race is gone by
  construction: no webview exists before the stores are managed. Visual
  confirmation (game chip + footer populated, no error box) on the owner's
  next launch.
- Three-lens adversarial review (tauri lifecycle vs vendored 2.11.5 source,
  behavior-regression hunt, docs accuracy): lifecycle + regression clean;
  docs nits fixed in the close commit.

## Status log
- 2026-08-02 — created from the packaged-exe bug report; root cause pinned
  to tauri's window-creation-before-setup order.
- 2026-08-02 — done: fix in c59b403, gates green, rebuilt exe boots clean.
