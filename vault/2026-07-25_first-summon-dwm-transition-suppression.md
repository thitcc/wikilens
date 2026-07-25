---
title: Suppress the DWM window-open transition so the first summon matches the rest
type: plan
status: done
created: 2026-07-25
updated: 2026-07-25
tags: [overlay, tauri, rust]
related: ["[[2026-07-25_summon-entrance-frame-sync]]", "[[2026-07-25_micro-motion-design-pass]]"]
commit: 75c5bba
---

# Suppress the DWM window-open transition so the first summon matches the rest

## Context / problem

The first summon after app launch plays a different entrance than every later
one: a fade + bottom-up rise instead of A-01's 120ms right-edge slide. The
investigation traced it to **Windows 11's DWM window-open transition**, not app
code: the overlay window is created hidden (`tauri.conf.json` `"visible":
false`), so the first summon is the HWND's first-ever presentation, and DWM
plays its own open animation (fade + short rise) over that first show. Later
`hide()`/`show()` cycles of the same never-destroyed HWND don't replay it,
leaving only the CSS entrance.

Verified empirically on-device: during the first summon the panel visually
rises ~56 physical px while `GetWindowRect` already reports the final dock
(purely visual, OS-level motion); summons 2–3 show zero vertical displacement.
The stylesheet has no `translateY` on `.panel` anywhere, and `window.rs` has no
first-show branch — nothing app-side can produce the motion. Nothing in
`src-tauri` sets `DWMWA_TRANSITIONS_FORCEDISABLED`, so the OS default applies.

## Goal / non-goals

- Goal: every summon — including the first after launch — plays exactly A-01
  (the 120ms right-edge slide + fade) and nothing else.
- Goal: the capture and debug windows get the same suppression — their shows
  are also plain `show()` calls on hidden-created windows, and a DWM fade+rise
  on the frozen capture snapshot would read as a glitch.
- Non-goal: changing A-01 itself (keyframe, duration, arming) — the entrance is
  correct from summon #2 onward.
- Non-goal: touching reduced-motion behavior. The CSS gate stays authoritative
  for app motion; suppressing DWM transitions only removes OS-added motion.

## Approach

1. Set `DWMWA_TRANSITIONS_FORCEDISABLED` (attribute 3, `TRUE`) via
   `DwmSetWindowAttribute` on each window's HWND **at setup, before any show**.
2. A small `#[cfg(windows)]` helper in `window.rs` (e.g.
   `disable_os_open_transition(&Window)`), called from `.setup()` for the
   overlay + capture windows and from `debug_window.rs` at creation.
3. FFI route: `windows-sys` as a `[target.'cfg(windows)'.dependencies]` entry
   (`Win32_Graphics_Dwm` + `Win32_Foundation`), HWND obtained from Tauri's
   `hwnd()`; exact crate/version reconciled against the existing lock file at
   implementation time.
4. Docs: note the suppression in DESIGN.md's A-01 inventory entry and add a
   first-summon line to `docs/smoke-checklist.md` (runtime-only surface — no
   automated coverage possible for the visual result).
5. Verify: `/check` for the code gates; manual smoke — first summon after a
   fresh launch must show only the right-edge slide.

Branch: `fix/first-summon-dwm-transition`.

## Decisions & trade-offs

- **Suppress at the HWND, not a warm-up show.** A cloaked (or off-screen)
  warm-up show at startup would also consume the one-time open transition, but
  it's fragile (flash risk, ordering races with positioning) and leaves the OS
  animation armed for any future window. The attribute is the documented switch
  for exactly this.
- **Failure is non-fatal.** If `DwmSetWindowAttribute` errors (old DWM state,
  remote session), log-and-continue — the app worked before; the transition is
  cosmetic.
- OS-wide "Animation effects" off already removes the transition; the attribute
  makes the behavior consistent regardless of user settings.

## Status log

- 2026-07-25 — created; investigation (workflow sweep + on-device differential
  capture) confirmed the DWM window-open transition as the cause.
- 2026-07-25 — done (75c5bba): `disable_os_open_transition` in `window.rs`
  (windows-sys, already in the tree at 0.61), applied to overlay + capture in
  `.setup()` and to the debug window in its `create()`. windows-sys 0.61 API
  notes: `BOOL` is gone from `Win32::Foundation` (plain `i32` now) and the
  raw call takes the attribute as `u32`. Docs: DESIGN.md A-01 entry,
  CLAUDE.md §5 gotcha, smoke-checklist first-summon item. `/check` green.
