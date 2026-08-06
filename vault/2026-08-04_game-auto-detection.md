---
title: Foreground-window game auto-detection
type: plan
status: active
created: 2026-08-04
updated: 2026-08-05
tags: [overlay, rust, frontend]
related:
  - "[[2026-07-02_scaffold]]"
  - "[[2026-07-19_hotkey-config]]"
  - "[[2026-07-05_builtin-game-registry-expansion]]"
  - "[[2026-07-05_user-added-game-wikis]]"
  - "[[2026-07-25_first-summon-dwm-transition-suppression]]"
commit:
---

# Foreground-window game auto-detection

## In simple terms

The player picks the game by hand today, every time they switch. But the
overlay is literally floating over the game — Windows can say which program
owns the window underneath. So on summon, WikiLens reads that program's
filename, looks it up in a small table, and *offers* what it found: a small
chip appears beside the game chip reading "Terraria", and one click switches.
It never switches on its own. If it recognises nothing, or if you ignore the
chip, nothing happens — the selection stays exactly where you left it, which is
the behaviour that exists today.

## Context / problem

The player picks the game by hand via the header chip on every switch. The
overlay already knows what it's floating over — the foreground window is the
game — so summoning could preselect the right wiki. Parked on the roadmap since
the scaffold ([[2026-07-02_scaffold]] listed it as a structure-for-later
non-goal, framed then as *window-title* matching).

Researched 2026-08-05 (14-agent sweep: Win32 mechanics with on-device
measurement, per-game process identity with adversarial re-verification, prior
art, two design critiques). The 2026-08-04 sketch survives in spirit — sample
Rust-side on summon, the manual pick stays the last word — but three of its
four load-bearing
clauses are wrong, one of them fatally. They are corrected below and the
correction is the point of this revision.

**The ship-blocker.** Steam appinfo 238960 (Path of Exile) and 2694490 (Path of
Exile 2) each declare exactly one Windows launch entry, and they are
byte-identical: `PathOfExileSteam.exe --nopatch`. `wiki/games.rs:85-96`
registers `poe` → poewiki.net and `poe2` → poe2wiki.net as two different wikis.
A basename-keyed table therefore either mis-routes every PoE2 question to the
PoE1 wiki *with genuine-looking PoE1 source links*, or — with rows for both —
makes both games permanently undetectable. A title tiebreak doesn't save it
either: `"Path of Exile"` is a prefix of `"Path of Exile 2"`. The only working
discriminator is the parent directory leaf (`config.installdir` is
`Path of Exile` vs `Path of Exile 2`). **The match key is `{exe basename,
parent directory}`, not a process name.**

**`GameWiki` is the wrong home for the rule, three times over.** It is
simultaneously the built-in registry entry *and* the on-disk `wikis.json`
record ([[2026-07-05_user-added-game-wikis]]), and
`games.rs:198 stored_form_omits_unset_namespace` exists specifically to hold
that file at `{id, name, api_url, page_url}`. The frontend never receives
`GameWiki` at all — it gets `GameInfo {id, name, custom}`. And `wiki/` is
portable MediaWiki logic; a Windows process rule doesn't belong in it.

**"(built-in and user-added)" can't be honoured.** `add_game(name, url)` takes
two strings and `probe::WikiCandidate` is derived entirely from the wiki's own
siteinfo — there is no UI, no probe output and no store field that could ever
populate a process name. A field on user entries would be a permanently-empty
slot in every user's `wikis.json`.

**"Manual pick stays the override" isn't a rule yet, and without one the
feature is actively annoying** — auto-applied, every re-summon reverts a
deliberate manual pick, including the common hide → re-summon-to-follow-up
loop, which would then answer the follow-up from the wrong wiki. Resolved on
2026-08-05 by inverting the default: the detection is **offered**, never
applied (see Decisions). That deletes the override problem instead of managing
it, and it is the decision the rest of the frontend design falls out of.

## Goal / non-goals

- Goal: summoning over a known game offers it as a **one-tap suggestion**
  beside the game chip. Accepting routes through the same `handleGameChange`
  path a manual pick uses (so the persisted selection can never disagree with
  the chip across a restart). Detection never changes the selection by itself.
- Goal: precision over recall, structurally. Ties resolve to no-match; the exe
  is a mandatory part of every rule; a title-only rule is unrepresentable by
  construction (it would match a browser tab open on the game's own wiki).
  Suggest-only already defuses the severe failure — a false positive is an
  ignorable label, not a wrong wiki answering fluently — but the discipline
  stays, because a chip that is often wrong trains the player to stop looking
  at it, and an ignored affordance is worth less than no affordance.
- Goal: the raw image path never leaves Rust. Only a resolved game id crosses
  IPC.
- Non-goal: auto-adding unknown games (the add-game probe flow stays
  deliberate); detecting while the overlay itself is foreground.
- Non-goal (v1): a settings toggle, a user binding UI, a `wikis.json` schema
  change, a `SetWinEventHook` foreground tracker. Each has a written reversal
  or follow-up sketch below.
- Non-goal: making detection work for user-added wikis. It structurally cannot
  in v1 — phase 2's learn-on-correct is the only mechanism that ever will.

## Approach

### Phase 0 — the foreground spike (gate; ships alone, nothing user-visible)

The [[2026-07-19_hotkey-config]] ABNT2 precedent applied: measure on-device
before writing the table, and paste the result into the Status log as the
primary source the table cites.

1. `src-tauri/src/detect/foreground.rs` — the complete Win32 probe, plus the
   pure `detect::reduce`, plus **one `debug::debug_enabled()`-gated stderr line
   per summon** at the top of `window::show_overlay`:
   `[wikilens] foreground: exe=… parent=… title=…`. No rules table, no
   matcher, no IPC change, no frontend change; `overlay://shown` stays `()`.
2. Protocol: `WIKILENS_DEBUG=1 npm run tauri dev`; for each reachable game,
   launch borderless/windowed, press Ctrl+`, copy the line. Also summon over a
   browser, the desktop, the Steam client, the tray icon, and immediately
   after hiding.
3. Three answers only the spike can give: whether **BattlEye** (Conan Exiles,
   a built-in) denies `OpenProcess`; whether a **Game Pass / MSIX** title
   resolves to the game or to `applicationframehost.exe`; and whether the
   summon hotkey fires *and takes keyboard focus* over an **elevated** window
   (if the panel appears but can't be typed into, that's a pre-existing
   WikiLens bug this feature merely exposes, and deserves its own doc).

**Gate: the table PR does not open until that Status log entry exists.**

### Phase 1 — v1

4. **Cargo features** — extend the existing `[target.'cfg(windows)']` block
   (`Cargo.toml:52`, windows-sys 0.61.2 already in the tree) with
   `Win32_UI_WindowsAndMessaging` + `Win32_System_Threading`, commented in the
   established style. Explicitly *not* `Win32_System_Diagnostics_ToolHelp`,
   `Win32_Graphics_Gdi`, or `Win32_UI_Accessibility`.
5. **`detect/foreground.rs`** — the only Windows-coupled file, following the
   `window.rs:85-123` / `keys.rs` idiom exactly (`#[cfg(windows)]` twinned with
   a `#[cfg(not(windows))]` stub returning `None`; function-local
   `use windows_sys::…`; raw ints; never panic). Sequence:
   `GetForegroundWindow` → `GetWindowThreadProcessId` →
   **`if pid == GetCurrentProcessId() { return None }`** →
   `GetWindowTextW` into a fixed `[u16; 512]` →
   `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` +
   `QueryFullProcessImageNameW` → `CloseHandle` immediately. Returns the raw
   full path and title; reduction happens on the pure side.
6. **`detect/mod.rs`** (pure, fully unit-tested) —
   `reduce(image_path, title) -> Foreground { exe, parent, title }`, splitting
   on both separators and lowercasing;
   `DetectRule { game_id, exe: &'static str, parent: Option<_>,
   title_prefix: Option<_> }` (all `&'static str`, so the table is a
   zero-allocation `&[DetectRule]`); and
   `match_game(rules, fg) -> Option<&'static str>`, deliberately **two-pass and
   order-independent** — filter to rules whose exe matches and whose
   constraints hold, score each survivor by constraint count, take the top
   tier, and return `None` unless exactly one `game_id` survives it.
   `title_prefix` is a *prefix*, never `contains`.
7. **`detect/rules.rs`** — the table below, with `const fn` one-liner
   constructors mirroring `wiki::games::wiki()`. Shipping gate, both required:
   the basename is **distinctive** (appears in no other rule; is not a generic
   host, launcher or bootstrap shim), **and** either the phase-0 spike
   confirmed it or ≥2 independent sources agree with no collision. Unconfirmed
   rows ship carrying `// researched, distinctive, unconfirmed on-device`.
8. **The seam: the top of `window::show_overlay`, before `position_top_right`**
   — not the `lib.rs` hotkey handler. All four `show_overlay` call sites are
   pre-guarded to a hidden overlay (`window.rs:133`, `window.rs:168`,
   `capture.rs:152`, `capture.rs:263`), so the PID guard inside the sampler
   covers every path with zero special cases, and there is no managed store and
   therefore no staleness. Cost is ~18 µs on a path that already does
   `MonitorFromWindow` + `GetDpiForMonitor` + 2× `SetWindowPos` +
   `SetForegroundWindow` + an IPC emit.
9. **Transport: widen the `overlay://shown` payload.** `ShownInfo
   { detected_game: Option<String> }` (camelCase serde), declared next to
   `EVENT_SHOWN` in `window.rs` — the `capture::AttachmentInfo` precedent. A
   struct, not a bare `Option`, so later summon-time facts join it instead of
   minting sibling events. In `api.ts`, `listen<ShownInfo | null>(…, e =>
   cb(e.payload ?? NO_SHOWN_INFO))` — that one `??` is what keeps the six
   existing bare `fireBackendEvent("overlay://shown")` call sites working and
   is why `src/test/backend.ts` needs no change at all.
10. **`src/App.tsx`** — the listener only *stores* the detection:
    `setDetectedGame(info.detectedGame)`, first in the handler, before
    `armSummon()` and outside the `SELECT_SUPPRESS_MS` branch. A plain
    `setState` has no stale-closure hazard, so **no `applyDetectionRef` and no
    `requestCaptureRef`-style indirection is needed** — the detection is
    validated against `games` at render time, where `games` is always current.
    That indirection was a cost of auto-switching; suggest-only deletes it.

    **The five rules:**
    - **S1** the chip renders only when `detectedGame` is non-null, is present
      in `games`, and differs from `selectedGame`. Equal → nothing to offer;
      unknown id (removed user wiki, stale rule) → nothing to offer; null (the
      overlay was summoned over a browser, the desktop, or the tray) → nothing
      to offer. One derived boolean, no branches, no state machine, no error
      box, no "unknown game" chip state.
    - **S2** clicking it calls `handleGameChange(detected)` — the ordinary
      manual path, so it persists to `wikilens.selectedGame` and pushes into
      Recent exactly like a menu pick — then returns focus to the prompt input
      (`closeMenu()`'s rationale: focus inside an unmounting subtree drops to
      `<body>` and deadens the keyboard). The chip then unmounts by S1, because
      detected now equals selected.
    - **S3** no dismiss, no "seen" memory, nothing persisted. Ignoring is free
      and costs one glance, and the chip is derived state — it disappears the
      moment the player accepts it or picks that game by hand, and needs no
      clearing on `overlay://hidden`.
    - **S4** hidden while an ask is in flight (`busy`), mirroring the existing
      `disabled={busy}` on `GameChip` — a switch mid-stream would leave the
      chip disagreeing with the streaming answer's sources.
    - **S5** it never takes focus on mount and is not part of the Esc layering
      (it is not a menu, so it stacks no capture-phase handler). The prompt
      input keeps focus through the whole summon, unchanged.

    Component: `src/components/GameSuggestion.tsx`, rendered immediately left
    of `GameChip` in the header on the existing `.quiet-chip` vocabulary with
    an accent treatment. **Name only, no monogram tile** — the header already
    carries brand + gear + game chip, and "The Elder Scrolls V: Skyrim" crowds
    it; truncate with ellipsis and carry the full name in `aria-label`
    (`Switch to …`). Entrance follows DESIGN.md's motion taste (small
    directional travel + fade, ease-out, 120–150 ms), never a pop. Any new
    size/radius/color is documented in DESIGN.md's frontmatter, never added
    silently. **Fallback if the header can't hold both at the narrow panel
    width:** move the chip to a slim row directly under the header — still one
    tap, still no new layer, no rule changes.
11. **Docs, same PR** — CLAUDE.md §2 file map (both `detect/` and
    `GameSuggestion.tsx`); §4's `overlay://shown` contract (no longer `()`,
    and its detected id is a **suggestion the frontend renders**, never
    something Rust applies); an **optional** rider on §4's "adding a built-in
    game = one `GameWiki` entry" bullet (a detect rule is optional; a game
    without one is fully functional and stays a manual pick); a §5 gotcha
    pinning the seam ordering. `docs/smoke-checklist.md` gains a "Game
    detection" section. Then `node .claude/skills/sync-agents/generate.mjs`.
12. **Instrumentation that stays** — the shipped `WIKILENS_DEBUG` line logs
    only the matched id (`[wikilens] detected minecraft`) plus a session
    hit/miss counter. The raw exe and caption stay in phase 0's form and are
    removed before PR 1 merges. Nothing about the foreground process ever
    enters a `debug://` payload, `history.json`, or a prompt. Under
    suggest-only the number that should decide whether the table grows is
    **acceptance**, not hit rate — a suggestion shown and ignored is the signal
    that a row is wrong or unwanted, and nobody has estimated either figure.

Branches: `feat/foreground-detect-spike`, then `feat/game-auto-detection`.

### The table (phase-0 spike confirms or downgrades each row)

| id | foreground exe(s) | constraint | conf. |
|---|---|---|---|
| `stardew` | `Stardew Valley.exe`, `StardewModdingAPI.exe` | — | verified |
| `corekeeper` | `CoreKeeper.exe` | — | verified |
| `conanexiles` | `ConanSandbox-Win64-Shipping.exe` | — | likely |
| `warframe` | `Warframe.x64.exe` | — | verified |
| `gw2` | `Gw2-64.exe`, `Gw2.exe` | — | observed on-device |
| `poe` | `PathOfExileSteam.exe`, `PathOfExile.exe`, `PathOfExileEGS.exe`, `PathOfExile_KG.exe` | **parent `path of exile`** | verified |
| `poe2` | *(the same four)* | **parent `path of exile 2`** | verified |
| `abioticfactor` | `AbioticFactor-Win64-Shipping.exe` | — | verified |
| `davethediver` | `DaveTheDiver.exe` | — | verified |
| `skyrim` | `SkyrimSE.exe`, `TESV.exe`, `SkyrimVR.exe`, `TESV_original.exe` | — | verified |
| `fallout4` | `Fallout4.exe`, `Fallout4VR.exe` | — | verified |
| `grounded` | `Maine-Win64-Shipping.exe`, `Maine-WinGDK-Shipping.exe` | — | observed on-device |
| `grounded2` | `Grounded2-WinGRTS-Shipping.exe`, `Grounded2-WinGDK-Shipping.exe` | — | verified |
| `terraria` | `Terraria.exe` | — | verified (vanilla only) |
| `minecraft` | `javaw.exe`, `java.exe` | **title prefix `minecraft`** | verified |
| `minecraft` | `Minecraft.Windows.exe` | — | verified |

Notes the table can't carry: **Grounded**'s codename is *Maine* and
**Grounded 2**'s platform tag is the unusual *WinGRTS* — neither is derivable
from the game name, and a substring rule on "Grounded" misses the first and
hits the second, which is how the two reach their two different wikis.
Grounded is the one row confirmed first-hand so far (2026-08-05): the probe
returned `Maine-Win64-Shipping.exe` with the caption `"Grounded"`, which also
upgrades its title from the research's "not observed" — though the row still
ships without a title constraint, since the exe alone is distinctive.
**Minecraft** is the one row where `title_prefix` is a required narrowing
constraint, not a tiebreak: `javaw.exe` is every Java app on the machine. Only
the bare prefix works — Forge renders `Minecraft* Forge 1.21.x` and NeoForge
`Minecraft NeoForge* 1.21.x`, so any tighter regex breaks both. Bedrock is a
GDK app since 1.21.120 and owns an ordinary Win32 window, which retires the
`ApplicationFrameHost` question. **Stardew**'s exe and title rules must never
be ANDed — SMAPI owns a *second* window in the same process, captioned
`SMAPI …`. **Terraria** has no usable title rule at all (the caption is a
randomly re-rolled, localised, mod-rewritable title message).

Never in the table (the exclusion list, to be pinned as a comment in
`rules.rs`): launchers and UE bootstrap shims — `FuncomLauncher.exe`,
Warframe's `Launcher.exe`, `SkyrimSELauncher.exe`, `Fallout4Launcher.exe`,
`AbioticFactor.exe`, `Grounded.exe`, `MinecraftLauncher.exe`,
`gamelaunchhelper.exe`; loaders — `skse64_loader.exe`, `f4se_loader.exe`,
`ModOrganizer.exe`, `Vortex.exe`; supervisors and servers —
`ConanSandbox_BE.exe`, `TerrariaServer.exe`, `*Server-Win64-Shipping.exe`;
generic hosts — bare `dotnet.exe`, and `javaw.exe` without its title
constraint; dead or fabricated names — `Warframe.exe` (32-bit, EOL 2019),
`KZW.exe`, `SkyrimSE_original.exe`, `PathOfExile*_x64*.exe` and `Client.exe`
(GGG-confirmed backwards-compat stubs), `Grounded2-Win64-Shipping.exe`.

### Documented gaps

**The parent leaf only discriminates for games whose binary sits in the install
directory.** Observed on-device 2026-08-05: Grounded's binary lives at
`…\Grounded\Maine\Binaries\Win64\Maine-Win64-Shipping.exe`, so its parent leaf
reduces to `win64` — generic across every Unreal game, and `wingdk` / `wingrts`
for the console-SDK targets. Path of Exile has the opposite shape
(`…\Path of Exile\PathOfExileSteam.exe`, the binary directly in the install
dir), which is exactly why the poe/poe2 split works. No v1 rule pairs a parent
constraint with an Unreal game, so nothing here is broken — but anyone
extending the table must not assume the parent leaf names the game. Doing that
for a UE title needs the same path-contains-segment rule tModLoader would need.

tModLoader runs as `dotnet.exe` at `<tModLoader>\dotnet\dotnet.exe` — the
parent leaf is `dotnet`, so even the parent mechanism doesn't rescue it;
covering it needs a path-contains-segment rule, a deliberate v2 extension.
Game Pass / MSIX basenames for Core Keeper, Dave the Diver and Abiotic Factor
are single-lineage or unsourced. Conan Exiles and Warframe both launch through
launcher processes that own their own windows, so detection is unavailable
during exactly the minutes a player is most likely to be reading a wiki.
Multi-monitor is the structurally worst case *and the one WikiLens targets*:
Windows has one foreground window system-wide, so a game plainly visible on
monitor 1 detects as nothing the moment the player clicks a browser on
monitor 2.

### Verify

`/check` for the code gates. Rust-side, 100% of the logic is pure and offline:
`reduce` splits both separators and — **the structural privacy pin** — never
emits a field containing `\`, `/` or `:`; `poe_and_poe2_split_on_the_parent_
directory` is the ship-blocker regression; `match_is_order_independent` asserts
`RULES` and a `rev()`ed copy agree, pinning the two-pass design against a
future "first match wins" rewrite; `unconstrained_rules_do_not_share_an_exe`;
`every_rule_targets_a_builtin_game` (deliberately only that direction — see
Decisions). One new `config_guardrails` pin,
`shown_info_serializes_exactly_the_known_fields`, styled on the `SettingsInfo`
twin. **No existing pin fails** — `SettingsInfo` is untouched and no `debug://`
payload gains a field; say so in the PR body, it's the check that proves v1
stayed in scope. Frontend `src/App.detect.test.tsx` covers S1–S5: a differing
known detection renders the chip; equal, unknown and null detections render
nothing; clicking it moves the game chip and writes both
`wikilens.selectedGame` and `wikilens.recentGames`; the chip is absent while an
ask is in flight; and — the regression that matters — a detection delivered
*before* `list_games` resolves still renders once the list arrives, which is
what render-time validation buys and a mount-time closure would fail.
`probe()` itself has no automated coverage on any platform — nothing calls it
in a test; it is ~30 lines of straight-line FFI feeding logic that is fully
covered. Then the on-device matrix in `docs/smoke-checklist.md`, whose
important item is the false-positive check: summon over a browser, the desktop,
the Steam client and the tray and confirm **no suggestion appears and the game
chip does not move**. Also: summon over a known game, ignore the suggestion,
hide, re-summon — the suggestion is still there and the selection is still
untouched.

### Open questions (owner's call)

- **Which games can the spike actually run against?** Only Guild Wars 2 is
  confirmed installed. Rows that can't be verified still ship if the basename
  is distinctive and ≥2 sources agree (worst case = silent miss) — but that's
  a policy to ratify, not an implementation detail.
- **Is phase 2's learn-on-correct acceptable?** It's the only durable answer to
  table rot and the only mechanism that will ever serve user-added wikis, but
  it sends a machine-local exe basename across IPC. v1 deliberately sends only
  a resolved game id.
- **Should the same sample also drive window placement?** `position_top_right`
  resolves the monitor from the *overlay's* `current_monitor()`, so on a
  multi-monitor setup the panel can dock to the screen the game is not on.
  Detection holds the game's HWND, one `MonitorFromWindow` away, and throws it
  away. It can't produce a wrong answer and it helps the 100% of user-added-
  wiki players detection can't serve — but it's a behaviour change to a tuned
  surface and belongs in its own doc, not smuggled into this PR.

## Decisions & trade-offs

- **Suggest, don't switch** (owner's call, 2026-08-05 — this doc originally
  specified auto-select). The detection renders as a one-tap chip and applies
  only on click. It converts the feature's one severe failure — a wrong wiki
  answering fluently under plausible-looking sources — into an ignorable label,
  and it pays for most of the frontend's complexity: the seven-rule matrix
  collapses to five much smaller ones, and the `overriddenDetectionRef`
  correction slot, the `{ manual: true }` option bag on `handleGameChange`, the
  Recent-dilution question and the `applyDetectionRef` stale-closure
  indirection all delete outright. Cost: one click per switch, and a chip that
  is often wrong becomes noise the player learns to skip — which is why the
  table's shipping gate stays strict even though the stakes dropped.
  Considered and rejected: auto-switch on the first summon after launch,
  suggest thereafter — two behaviours to explain, one of them still capable of
  the severe failure.
- **The choice is frontend-only.** PR 0 and every Rust deliverable in PR 1 —
  the probe, `detect/`, the rules table, `ShownInfo`, the guardrail pin — are
  identical either way; only `App.tsx` and the new component differ. Worth
  recording because it means the phase-0 gate does not need re-running if the
  owner ever reverses this, and because it keeps the backend free of any
  opinion about how the suggestion is presented.
- **`OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` over a Toolhelp
  snapshot.** Measured on this machine: 18 µs vs 9.18 ms — 510×, of which
  7.28 ms is the kernel call itself, before a single `PROCESSENTRY32W` is read.
  Toolhelp is also strictly *less* informative: it yields `szExeFile`
  (basename only), which is exactly the field that cannot separate `poe` from
  `poe2`. And it rescues nothing — every measured denial (159 of 331 pids) was
  a SYSTEM or protected process that will never own a game's foreground window.
  PQLI specifically: it reaches *more* processes than `PROCESS_QUERY_INFORMATION`
  (172 vs 151 measured) and is the one query right Microsoft's process-rights
  page does not deny to protected processes.
- **Anti-cheat posture is an invariant, not an implementation detail.**
  Published BattlEye analysis documents both TOPMOST-window enumeration
  (hunting overlay cheats) and handle enumeration against the game process, and
  Conan Exiles is a built-in. So: open → query → `CloseHandle` in
  microseconds, **never cache the HANDLE**, never request
  `PROCESS_QUERY_INFORMATION` or `PROCESS_VM_READ` (the rights that get
  scored), and degrade a denial to `None` — never an error box, never a retry.
  Reading a process *name* is what Task Manager, OBS game capture and every
  overlay do; this stays on that side of the line.
- **The rule lives in `src-tauri/src/detect/`, declared beside `capture` and
  `window` — not under `wiki/`, not on `GameWiki`.** Reasons in Context. It
  also keeps `wiki/` portable and the pure matcher compilable on any host.
- **Widened `overlay://shown` over a sibling `game://detected` event.** Tauri's
  own docs decline to guarantee event ordering; building the suggestion on an
  ordering coincidence would race `armSummon()`. A command invoked on shown was
  also rejected — it adds an IPC hop *after* the panel is animating (the chip
  visibly re-labels mid-entrance) and needs a `backend.ts` default handler or
  every existing summon test throws `unmocked command`.
- **Sample in `show_overlay`, not in the hotkey handler.** The hotkey seam
  misses `commands::show_overlay` → `show_overlay_if_hidden` (the capture
  flow's "this model can't read images" surface, where the reading is valid);
  it forces a managed store to bridge the write/read split, creating a second
  source of truth for "which game" alongside `App.tsx`'s `selectedGame`; and
  that store is stale by construction — a later tray summon would emit a
  detection sampled minutes ago over a game since closed.
- **The tray path gets no special case.** It samples the shell, matches
  nothing, emits `null`, and S1 renders nothing — so the panel opens on the
  persisted pick, which is both simpler and more useful than any detection
  attempt. Do not filter for `Shell_TrayWnd`.
- **No settings toggle in v1** (precedent:
  `2026-07-29_keys-are-not-a-mode-choice`). It would cost a `Persisted` field,
  a `SettingsInfo` field, an edit to the `settings_info_serializes_exactly_the_
  known_fields` pin, a `SettingsMenu` row with its a11y and three test
  fixtures — for a suggestion whose worst case is one wrong chip label the
  player fixes with the chip they already use. The reversal is pre-written:
  `auto_detect_game: Option<bool>` on `SettingsFile` (the `#[serde(flatten)]`
  extra map already proves an older binary won't strip it), `resolve_auto_
  detect` following `resolve_mode`, `set_auto_detect_game` following `set_mode`
  exactly, read in `show_overlay`.
- **The table has no rot signal and cannot have one.** Unlike `GOLDEN_CASES`
  (`wiki/mod.rs:119` checks them live), a detect rule needs the game installed
  and CI has zero games; an unmatched exe is byte-identical in behaviour to
  "the player is not in a game". This is why the table test runs only in the
  reverse direction — every rule names a registered game, but a game is *not*
  required to have a rule. The twin would turn "add a wiki" from a 4-line PR
  into "buy the game", whose pressure release is an invented exe name, i.e.
  exactly the failure this design exists to eliminate. The registry already
  rotted once inside the research window: Conan Exiles Enhanced (2026-05-05,
  UE4→UE5) renamed its binary, and tModLoader stopped being `tModLoader.exe`.
- **Privacy contract.** `QueryFullProcessImageNameW` returns a full path
  embedding the user's install layout and often their username, and captions
  carry server addresses and save names. The path is reduced to two lowercased
  leaves on the pure side — neither can contain a separator or a drive letter,
  and a unit test asserts it — only a resolved game id crosses IPC, and the raw
  strings appear only in phase 0's stderr line.
- **Refused outright, so they aren't re-litigated:** a fullscreen-geometry
  guard (re-opens the physical/logical pixel hazard §5 pins to
  `position_top_right`, to filter a case where the overlay can't be summoned
  anyway); an `ApplicationFrameHost` child-window walk (Bedrock 1.21.120+ is
  GDK; the Game Pass builds of Grounded/Skyrim/Fallout are full-trust Win32 and
  own their own HWNDs); command-line/PEB reading to disambiguate Minecraft Java
  (needs `PROCESS_VM_READ`, the most anti-cheat-suspicious right available —
  the `javaw.exe` + title-prefix rule covers it); a runtime fetch of Discord's
  `applications/detectable` (12.3 MB, unrelated to wikis, and §4 routes every
  client through `http::build_client()` — build-time derivation only, if ever).
- **`SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` deferred, not rejected.** It
  would fix the tray path and take detection off the summon path entirely, and
  needs no elevation or UIAccess — but it buys two minority behaviours for an
  unsafe process-global (`WINEVENTPROC` is a bare `extern "system" fn` that
  can't capture the `AppHandle`), an unhook lifecycle, and a new silent-failure
  mode. Shape v1's sampler so the hook can later become a *second* writer
  calling the same entry point: the PID guard, the never-clear rule and the
  read site all stay where they are.

### Deferred, in order

1. **Learn-on-correct** — carry the exe basename alongside `detectedGame`;
   when the player picks a game from the menu while a suggestion is showing,
   write `localStorage["wikilens.gameByExe"][exe] = gameId` and consult it
   before `RULES`. ~15 lines of frontend, zero backend, zero new store. It
   repairs Conan-Enhanced-class rot in one click, covers games nobody
   researched, and is the only mechanism that ever serves user-added wikis.
   Suggest-only makes the correction signal cleaner than it would have been
   under auto-switch: a menu pick with a suggestion on screen is an unambiguous
   "not that one", where under auto-switch it was tangled with undoing a
   change the app had already made. It also resolves the standing case of a
   player who deliberately wants Grounded 1's wiki while playing Grounded 2 —
   today the suggestion would sit there forever; learned, it stops appearing.
   Separate PR because it widens IPC from a resolved id to a machine-local
   filename and wants its own guardrail pin.
2. **`scripts/refresh-game-exes.mjs`** — derive the table from Discord's
   `applications/detectable` at maintenance time (live, unauthenticated,
   ETag/304-capable, contains all 15 built-ins and disambiguates poe/poe2 by
   directory). Undocumented and unsupported upstream, so a convenience, never a
   dependency. Own tooling PR with `npm run test:node` coverage.
3. **The settings toggle**, only if a user asks (shape above).
4. **The WinEvent hook**, as its own `status: idea` doc.

## Status log

- 2026-08-04 — created; migrated from the CLAUDE.md §6 roadmap when the
  section retired in favor of vault idea docs.
- 2026-08-05 — researched (14-agent sweep with adversarial re-verification of
  the exe table and two design critiques); Approach rewritten from a
  one-paragraph sketch to a two-PR plan. The sketch's process-name key was a
  ship-blocker: PoE 1 and PoE 2 declare byte-identical Steam launch entries
  (`PathOfExileSteam.exe --nopatch`, verified from appinfo 238960 and 2694490),
  so the key became `{exe, parent directory}`. Also corrected: the rule can't
  live on `GameWiki` (it doubles as the `wikis.json` record and the frontend
  never sees it), user-added entries have no channel that could supply a
  process name, and "manual pick stays the override" needed a stated rule
  rather than an assumption. Win32 route decided by measurement on this machine
  (`OpenProcess` 18 µs vs Toolhelp 9.18 ms), and the seam pinned to the top of
  `show_overlay` — sampling at or after `set_focus()` reads WikiLens's own PID
  and detection silently never fires.
- 2026-08-05 — owner picked **suggest over auto-switch**: the detection is
  offered as a one-tap chip beside the game chip and applies only on click.
  Goal, In-simple-terms, step 10 and the frontend tests rewritten; the R1–R7
  matrix became S1–S5, and the override slot, the `{ manual: true }` option
  bag, the Recent-dilution open question and the `applyDetectionRef`
  indirection were all deleted rather than redesigned. Rust half untouched — the
  choice is frontend-only, so the phase-0 gate stands as written.
- 2026-08-05 — started (status → active). **PR 0 built** on
  `feat/foreground-detect-spike`: `detect/foreground.rs` (the complete Win32
  probe, `#[cfg(windows)]` twinned with an honest `None` stub),
  `detect/mod.rs` (`Foreground` + the pure `reduce`, 6 unit tests incl. the
  `reduce_never_emits_a_path` privacy pin), the two windows-sys features, and
  the `WIKILENS_DEBUG`-gated `log_foreground_spike()` at the top of
  `show_overlay`. No rules table, no matcher, no IPC change, no frontend
  change — `overlay://shown` still emits `()`.
  windows-sys 0.61.2 verified against the vendored source before writing the
  FFI: everything is a raw alias (`HWND`/`HANDLE` are `*mut c_void`, `BOOL` is
  `i32`), `OpenProcess` signals failure with **NULL**, and both new features
  transitively imply `Win32_Foundation`. One refinement over this doc's
  snippet: the spike also prints `foreground: none`, because three of the
  protocol's steps are negative cases and silence would be
  indistinguishable from the gate being off. One implementation trap the plan
  hadn't named: `C:\game.exe` reduces its parent to `"c:"` unless drive
  specifiers are dropped, which would have put a colon through the privacy pin
  — handled, and pinned by `reduce_drive_root_parent_is_none`.
  First live reading, incidental (Grounded happened to be foreground while the
  tests ran): `exe="maine-win64-shipping.exe" parent="win64" title="Grounded"`
  — recorded in the reduced form deliberately, because the raw path carried the
  user's home directory and the reduction is what strips it. It confirms the
  `grounded` row and its caption first-hand, and it exposed a limitation the
  plan had not named: for Unreal games the parent leaf is `win64`, not the game
  — see Documented gaps.
  **Next: the owner runs the spike protocol and pastes the observed lines
  here. PR 1 stays closed until that entry exists.**
- 2026-08-05 — first spike readings, on-device from `npm run tauri dev` with
  `WIKILENS_DEBUG=1` (reduced form only — the raw paths carry the user's home
  directory):
  - `exe="maine-win64-shipping.exe" parent="win64" title="Grounded"` —
    **`grounded` confirmed**, exe and caption both.
  - `exe="code.exe" parent="microsoft vs code" title="Claude Code - wikilens -
    Visual Studio Code"` — the negative case, and the one that matters most:
    a non-game reads cleanly, no rule claims `code.exe`, so the matcher will
    return `None` and the chip will stay put. False positives are the only
    failure that costs anything.

  Confirms the plumbing end to end: the probe fires on every summon, the PID
  guard keeps WikiLens out of its own reading, and the reduction strips the
  path. 1 of 15 built-in rows is now on-device confirmed.
- 2026-08-05 — owner ratified **shipping the full table** rather than waiting
  for more on-device confirmations. Rows that only meet the research half of
  the gate carry `// researched, distinctive, unconfirmed on-device`
  (`conanexiles`, `abioticfactor`). The argument that carried it: suggest-only
  already downgrades a wrong row from "answers from the wrong wiki" to "an
  ignorable chip", and a distinctive-but-wrong basename fails as a silent miss,
  which costs nothing.
- 2026-08-05 — **PR 1 built** on `feat/game-detection-suggestion` (stacked on
  the spike branch until PR 0 merges). Rust: `DetectRule` +
  `match_game` (two-pass, order-independent, ambiguity → `None`),
  `detect/rules.rs` (16 rows over 15 games), `ShownInfo` widening
  `overlay://shown` from `()`, and the `shown_info_serializes_exactly_the_
  known_fields` guardrail pin. The spike's raw exe/caption line became
  `detected <id>` plus a session hit/miss tally — the raw strings no longer
  leave `detect`. Frontend: `GameSuggestion.tsx`, an accent chip left of the
  game chip, rendered from derived state (S1–S5) so there is nothing to
  dismiss, nothing to persist, and nothing to clear on hide. Design: A-05
  `suggestion-arrive` admitted to DESIGN.md's motion inventory (4px from the
  right, the direction of the chip it fills).
  Two things worth remembering. The `applyDetectionRef` the plan specified
  turned out to be **unnecessary** — storing the detection with a plain
  `setState` and validating against `games` at *render* time sidesteps the
  mount-only-closure hazard entirely, so the ref, the `{ manual: true }`
  option bag and the override slot all stayed deleted. And the planned
  `detected_game_ids_are_all_offerable_to_the_frontend` pin was dropped as
  genuinely redundant: `list_games` takes `State<'_, _>` (unconstructible in a
  unit test), and `every_rule_targets_a_builtin_game` already gives the
  guarantee, since `list_games` always emits every built-in.
  `/check` green: tsc, 201 Vitest (8 new), 55 node, clippy `-D warnings`,
  327 cargo (27 new).
