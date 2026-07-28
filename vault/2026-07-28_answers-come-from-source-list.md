---
title: One answer-source list — merging Model source and API keys in Settings
type: plan
status: done
created: 2026-07-28
updated: 2026-07-28
tags: [frontend, overlay, llm]
related:
  [
    "[[2026-07-26_default-mode-and-byo-api-keys]]",
    "[[2026-07-26_api-key-storage-dpapi]]",
  ]
commit: adca15d
---

# One answer-source list — merging Model source and API keys in Settings

## Context / problem

An impeccable critique of the Settings panel (`/impeccable critique`, snapshot
`.impeccable/critique/2026-07-28T04-27-56Z__src-components-settingsmenu-tsx.md`)
scored it **21/40** and found seven verified defects, four at P2. The detector
came back clean — every finding lives in the layer a text-matching rule engine
can't see.

The shape of the problem: three sections were each solved separately and
stacked, so the card carries three row vocabularies, four left edges, and two
button anatomies. Concretely, what a first-run player meets is a heading
("Model source") over two unglossed words, then a second heading ("API keys")
over **three anonymous `type="password"` fields** — one per registry provider,
identified only by a placeholder the first keystroke erases, stacked with zero
vertical gap so their hairlines double into a 2px seam. The panel's shape
follows the *registry*, not the player, who realistically uses one provider.

Two structural incoherences compound it. In **Default mode** the key section
renders identically and stays fully interactive, so a pasted key produces a
"Key set" badge and zero behaviour change. And on an install without
`WIKILENS_DEFAULT_*`, the permanently-disabled "Default" row is pixel-identical
to the live row below it — `.model-row` is the one interactive row style with no
`:disabled` rule and no `:not(:disabled)` hover guard — so it lights up under
the cursor and eats the click.

The card is also the only surface where the product could ever explain what an
API key is or where to get one, and it says nothing — forcing the alt-tab
WikiLens exists to abolish. `App.tsx` even delegates that explanation here in a
comment ("the explanation lives in the Settings panel's Model source section"),
as does consequence 12 of [[2026-07-26_default-mode-and-byo-api-keys]].

## Goal / non-goals

- **Goal:** one exclusive list, headed "Answers come from", where every row is a
  source and exactly one wears the accent check — the row answers are actually
  coming from right now. Picking an unkeyed provider is what opens its key
  field, in place, one at a time.
- **Goal:** close the four P2 findings. Three of the four shape approaches
  independently declared the focus and `.model-row:disabled` fixes to be
  dependencies of any restructure, so they ship here.
- **Non-goal:** the footer chip. Settings will say "Built into WikiLens" while
  the footer static chip still reads "Default". One word, one state, its own
  tests — the vocabularies should converge later, on the footer's terms.
- **Non-goal:** touching the Shortcuts section. It is the critique's top praise
  (the three-layer Esc, the suspend/resume lifecycle) and ships byte-identical.
- **Non-goal:** a fourth Esc layer. Refusing one is why the nine recorder tests
  survive a structural rewrite of the other half of the card.

## Approach

Shaped with `/impeccable shape`: four independent structural approaches, a
three-lens judge panel (mid-game speed · house-law conformance · build risk),
then synthesis. The winner took second on raw total (20/30 vs 21) but won two of
three lenses, including the one PRODUCT.md privileges — speed is the product
metric, and it is the only direction that changes what the player reads *first*.

**Structure.** Two sections instead of three: the source list, then Shortcuts.
Rows are `.source-row` wrappers holding a pickable `.model-row` plus, on keyed
providers, a sibling `.remove-btn` (the `.custom-game-row` pattern from
AddGameMenu — a Remove button cannot nest inside the row's own `<button>`). The
key form is a plain sibling `<div>` mounted after the row that owns it: no fill,
no border, no shadow, so the Two Altitudes Rule is untouched by construction.

**State** is derived, not stored: `showBuiltIn`, `sourceId` (which row wears the
check), plus exactly two real state fields — `openKey: string | null` (at most
one form open) and `sourceError: { rowId, message }`, which collapses today's
separate `modeError` and `keysError` so a failure lands under the row that
produced it. `drafts: Record<string, string>` becomes one `draft: string`, since
only one field can exist — shorter key-material residency, one rule.

**Commit paths.** Built-in row → `set_mode("default")`. Keyed provider → mode to
custom if it isn't already, then the provider pick. Unkeyed provider → **no
IPC**, just toggle the form. Save → `set_api_key` → pick → `set_mode` if needed
→ `onSaved` → `onKeysChanged`, in that order, so App's provider re-fetch runs
after the mode is custom and finds the pick already stored.

**Order of work:** frontend core (SettingsMenu, App props, styles, the new
`src/providerHelp.ts` host map) → tests → Rust copy + guardrail assertions →
docs sweep → `/check` → `/vault-lint`.

## Decisions & trade-offs

- **The merge is the point, and it is the expensive choice.** The build-risk
  judge scored it 3/10 and was right: ~350 lines of product code and docs plus
  ~190 of test churn, against the runner-up's 3 test edits and zero Rust. The
  runner-up ("One Field at a Time") collapsed the three fields via derived state
  but left the jargon header standing and *raised* the card to four row
  vocabularies — the named defect relocated, not removed. Buying a cheap build by
  leaving the headline problem in place is the wrong trade when the metric is
  time-to-back-in-game.
- **"One Slot" was disqualified outright.** It pre-aimed and pre-focused a single
  field, optimising exactly the gesture — blind Ctrl+V — that stores a key
  against the wrong vendor. Because keys never render back, that failure is
  silent when it happens, deferred to ask time, and unverifiable in-app. The
  extra click *is* the decision the critique asked for: the field only ever
  exists under the name that owns it.
- **Saving a key also commits the source.** Flagged as the one dangerous item by
  the build judge, on the word "silently". Rejected on the baseline: today a
  player leaves Default with one click on "Custom API" and no key at all.
  Requiring a stored key first makes Default *harder* to leave, and the accent
  check visibly moves to the clicked row in the same frame.
- **No two-step Remove.** The critique offered it at P3; this kills the same trap
  geometrically instead — Save lives inside the expansion, "Remove key" sits on
  the row above with a note line between them, so the footprint collision the
  finding is actually about no longer exists. Remove stays one click.
- **The vendor-link host map lives in TS (`src/providerHelp.ts`), not Rust.**
  Putting `key_url` on `Provider` would widen `KeyStatus`, and
  `config_guardrails::key_status_serializes_exactly_the_known_fields` exists
  precisely to force review before that happens — a hyperlink is the worst thing
  to spend that pin on. Named cost: a fourth provider added by one `providers.rs`
  entry gets a row, a working field, and a generic sentence with no link until
  someone edits the map.
- **Rust copy rides in this PR, not a follow-up.** Two assertions pin strings
  this change deletes — `config_guardrails.rs:336` (`"API keys"`) and
  `target.rs:213` (`"Custom API"`) — so CI goes red otherwise, and a doc pointing
  at a deleted section is worse than a renamed one.
- **Owner rulings (2026-07-28):** ship the merged list; keep the vendor console
  links (minting a key genuinely cannot happen in-panel, and naming a host
  without linking it is the worse of the two honest options); use "Built into
  WikiLens".

## Status log

- 2026-07-28 — created. Critique run (21/40, 7 verified findings, 4×P2), shape
  run completed (4 approaches, 3 judges, synthesis), owner confirmed the merged
  direction and both copy rulings. Implementation starting.
- 2026-07-28 — landed (`adca15d`). All four P2 findings closed. Nine recorder
  tests stayed byte-identical (refusing a fourth Esc layer is what bought
  that); `SettingsMenu.test.tsx` grew to 29 tests, and every security
  assertion carried over verbatim — the once-and-only-once
  `callsTo("set_api_key")` equality, the `queryByLabelText` null check on a
  keyed provider, the masked-field attributes. `/check` green: 146 Vitest, 38
  Node, 290 Rust, clippy `-D warnings`, `tsc` clean.
- Two bugs the test agent caught in review, both fixed before the commit: the
  in-flight label was driven by a panel-wide `action`, so with two keyed
  providers both Remove buttons read "Removing…" for one click (now tagged
  with `actionRow`); and the zero-source lead line said "…and add its key"
  even on installs offering the built-in row, which needs no key.
- **Follow-ups, deliberately not in this PR.** The footer chip still reads
  "Default" while the panel says "Built into WikiLens" — one word for one
  state, on a surface with its own tests; the vocabularies should converge on
  the footer's terms. And `CLAUDE.md` crossed the sync-agents size warning
  (31174 → 31978 bytes, against Codex's 32768 cap) — the file needs a trim
  pass of its own before the next doc-heavy change.
