---
name: proof-sheet
description: Run the proof-sheet loop for front-end changes with an open visual choice — variant specimens as one self-contained HTML in .impeccable/critique/ (gitignored), on real product tokens, both themes side by side, published as an artifact; picks are asked mid-session and recorded in the vault plan doc. Invoke when a front-end change has an open visual fork or the user asks for variants.
allowed-tools: Read, Write, Edit, Bash(node:*), Artifact, AskUserQuestion
---

# proof-sheet

The samples-first design workflow: before implementing a visual change whose
look is genuinely open, build the variants as specimens the user can see and
feel, ask for the picks in the same session, record the answers, then
implement. The reusable scaffold is
`.claude/skills/proof-sheet/templates/sheet-skeleton.html`; the precedent with
four recorded iterations is the status log of
`vault/2026-08-13_settings-stepper.md`.

## When to run

- A front-end change carries an **open visual choice** — glyph, spacing,
  border treatment, motion, alignment — that the design register doesn't
  already settle.
- The user asks to **see variants** of something visual.
- Manually, as `/proof-sheet`.

Not for mechanical changes, refactors, or bugfixes with no visual fork: a
change whose look is already determined needs no sheet.

## The loop

1. Build the sheet (below) into `.impeccable/critique/` (gitignored).
2. Publish it as an artifact and hand over the link.
3. Ask for the picks **mid-session** with AskUserQuestion — one question per
   axis, the recommended option first.
4. Record picks and declines in the vault plan doc's status log.
5. Implement the picks.

Never end the turn to wait for a fresh prompt while a pick is pending —
sheet → picks → implement is one continuous loop.

## Building the sheet

- Start from `.claude/skills/proof-sheet/templates/sheet-skeleton.html`; one
  file per decision, named after it (e.g. `stepper-border-sheet.html`).
- Copy the **current** token values from `src/styles.css` into the
  `.t-default` / `.t-micro` blocks at build time — the skeleton ships names
  only, because a stale-token sheet lies about what the picks will look like.
- Every variant renders under **both themes side by side** — no theme toggle,
  by design; the themes are compared, not switched.
- 3–4 axes per sheet at most, each with a one-or-two-sentence rationale in
  its `.note`; more forks means more sheets.
- Carry **one recommended variant per axis** into the questions.
- Make it interactive where feel matters — a cycling stepper, toggleable
  motion — and static where it doesn't.
- Don't re-show variants the design register already ruled out (DESIGN.md,
  the decision docs) without new cause: the instrument-trim rule killing the
  position ticks is the canonical example.

## Recording picks and declines

One status-log line in the vault plan doc: date — which sheet — the picks
with a one-clause rationale each — and the **explicit declines**. Declines
are recorded with the same weight as picks: an unrecorded decline invites a
future session to "complete" what was deliberately refused. Then run
`node .claude/skills/vault-lint/lint.mjs`.

## The design hook and sheet chrome

Sheet chrome (the `--sheet-*` skin, the fake game backdrop) is deliberately
non-product, so the impeccable hook's palette findings on sheet files are
expected false positives — `.impeccable/critique/` and this skill's
`templates/` are suppressed via `detector.ignoreFiles` in
`.impeccable/config.json`. If a finding still surfaces on a sheet, classify
it as intentional; never "fix" sheet chrome toward product tokens.

## Template maintenance

The skeleton ships token *names* and structure, never values, so token drift
can't stale it. True it up only when the product renames or grows a custom
prop the specimens consume, or when the sheet anatomy itself changes.
