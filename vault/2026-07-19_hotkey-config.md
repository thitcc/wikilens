---
title: Hotkey config — the first settings surface
type: plan
status: active
created: 2026-07-19
updated: 2026-07-19
tags: [hotkey, frontend, rust, overlay]
related: ["[[2026-07-18_ask-lifecycle-fix-pass]]", "[[2026-07-02_scaffold]]"]
commit:
---

# Hotkey config — the first settings surface

## Context / problem

The summon hotkey is a hardcoded global Shift+C. On Windows `RegisterHotKey`
swallows it system-wide: a capital `C` can never be typed into the prompt, and
an accidental press hides the panel mid-draft (the capital-C draft-loss trap).
[[2026-07-18_ask-lifecycle-fix-pass]] shipped the interim ~20-line select-all
suppression and deferred the root fix to this doc: the settings store + key
recorder — "the real capital-C fix, promoting the §6 roadmap item; first item
of a growing config panel". Wanted: both global shortcuts (summon and capture)
configurable from an owned overlay surface, persisted Rust-side, with the
default summon key moved off the Shift+letter class entirely.

## Goal / non-goals

- Goal: a settings surface (header gear → owned "Shortcuts" popover) whose
  first tenants are the summon and capture hotkeys, each with a key recorder
  row — owned UI, no native controls.
- Goal: Rust-side `SettingsStore` (`settings.json` in the app-data dir,
  `UserWikiStore` pattern), get/set commands, live re-registration of the
  global shortcuts, conflict validation between the two combos.
- Goal: default summon becomes Ctrl+` (`Ctrl+Backquote`, the physical key left
  of 1); stored choices are honored over defaults. Acceptance: the capital-C
  trap is dead at the root with the new default.
- Non-goal: any other settings tenant (model/game prefs stay in localStorage);
  a Rust-side auto-resume watchdog for the recorder's suspend state; surfacing
  startup registration failures in `get_settings` (tray tooltip remains that
  surface).

## Approach

1. ABNT2 gate spike (before code): confirm scancode 0x29 ↔ VK_OEM_3 on this
   machine's layouts, since global-hotkey pins `Code::Backquote → VK_OEM_3`
   with no scancode path.
2. Rust foundation: `settings.rs` store (load ladder mirroring
   `wiki/user.rs` — corrupt→`.bak`, unreadable→refuse-mutations,
   persist-then-commit; `#[serde(flatten)]` extra map so future keys survive);
   `hotkey.rs` rework (`default_summon`/`default_capture`,
   `parse_accelerator`/`to_accelerator`/`display_label`, `apply_change` with
   rollback, suspend/resume registration helpers); `lib.rs` handler matches
   via `SettingsStore::role_of` and `.setup()` loads the store before
   registration; tray tooltip derives from the configured combo.
3. Commands: `get_settings`, async `set_hotkey(role, accelerator)` (parse →
   conflict check → writability first → probe-or-swap registration →
   persist-then-commit → tooltip), `suspend_hotkeys`/`resume_hotkeys` for the
   armed recorder; mirrored in `api.ts`/`types.ts`/test backend.
4. Frontend: pure `src/hotkeys.ts` (KeyboardEvent→accelerator, validation with
   the Shift+letter trap rationale, label tables paired-by-comment with the
   Rust twins); `SettingsMenu.tsx` on the AddGameMenu template (capture-phase
   Esc; armed row = accent, three-layered Esc; suspend on arm, resume on every
   exit path incl. unmount and `overlay://hidden`); header gear chip;
   dynamic prompt placeholder + capture chip title.
5. Docs: README, CLAUDE.md (§1, §2 map, §4 events, §5 gotcha kept as rationale
   marked resolved, §6 roadmap swap), smoke checklist (§2 inverts — capital C
   now typeable), ai-workflow.html; sync-agents regen.
6. Single PR off `feat/hotkey-config`; behavior change (default summon key)
   called out explicitly.

## Decisions & trade-offs

- **Both hotkeys configurable now over summon-only** (user pick): second
  recorder row + conflict validation is modest extra work and completes the
  surface.
- **Header gear over footer chip / game-menu row** (user pick): settings is
  panel-level, not model machinery; header is brand-left/game-chip-right, so
  the gear sits beside the brand and GameChip keeps the right edge.
- **Ctrl+` as default over Ctrl/Alt+letter**: no letter is swallowed, the key
  left of 1 is reachable one-handed, and on the active ABNT2 layout it's a
  dead key — a Ctrl combo there can't collide with typing at all.
- **Store accelerator strings, parse on load**: one canonical form
  (`Ctrl+Backquote`) shared by the recorder, the file, and IPC — equality is
  plain string compare; unparseable fields fall back per-field to defaults
  without rewriting the file.
- **One generic registration-failure message**: the plugin flattens
  `AlreadyRegistered` into a string error, so no variant branching — "another
  app may already be using it" covers all failures.
- **Suspend-while-recording via unregister/re-register**: the OS shortcut
  would otherwise fire mid-recording; `overlay://hidden` disarms as the safety
  net. Accepted residual: a webview crash while suspended leaves hotkeys dead
  until restart (tray still works).

## Status log

- 2026-07-19 — created; scope + both-hotkeys and header-gear decisions from
  plan-mode Q&A.
- 2026-07-19 — ABNT2 gate spike PASSED: on all three installed layouts
  (2× pt-BR ABNT2 incl. active, 1× en-US) scancode 0x29 ↔ VK_OEM_3 both
  directions (`MapVirtualKeyEx`), so `Ctrl+Backquote` fires on Ctrl + the key
  left of 1 here; browser `e.code` for that position is `"Backquote"` —
  recorder, store, and OS registration agree. Ctrl+` stands; no decision doc
  needed.
