---
description: Scaffold a new planning-vault plan doc from vault/templates/plan.md
argument-hint: <short description or kebab-slug>
allowed-tools: Read, Write, Edit
---
Create a new plan doc in the WikiLens planning vault from `vault/templates/plan.md`.
Do the mechanical scaffolding so no frontmatter is ever hand-edited.

Steps:
1. Derive a **kebab-case slug** (for the filename) and a **human-sentence title**
   from "$ARGUMENTS" and what we've just been discussing. If "$ARGUMENTS" is empty,
   derive both from the recent discussion — don't stop to ask.
2. Filename = `vault/<today>_<slug>.md`, where `<today>` is today's date as ISO
   `YYYY-MM-DD`. If that file already exists, **STOP and report it** — filenames are
   permanent IDs; never overwrite or rename an existing doc.
3. Copy the template body and write **clean** frontmatter (drop the template's
   teaching `#` comments — real docs carry values only, e.g.
   `vault/2026-07-03_commit-field-forms.md`):
   - `title`: the human sentence (also the `#` H1)
   - `type: plan`
   - `status: active` if we're starting the work now; `idea` or `todo` if we're just
     capturing it for later
   - `created` / `updated`: today's date
   - `tags`: only tags already in the registry in `vault/index.md` — never invent
     one; leave `[]` if none fit
   - `related`: quoted wikilinks (e.g. `"[[2026-07-02_scaffold]]"`) if any, else `[]`
   - `commit`: leave empty — set it on close, once the code is committed
4. Fill **Context / Goal / Approach** from what we discussed; leave the remaining
   sections as the template's prompts.
5. Add the doc to the manual index list in `vault/index.md` under **Active / blocked**
   (or the matching section if status isn't active).
6. Report the created path and its frontmatter.

Reminder for while we work: if a real fork shows up — a choice with an alternative
we're deliberately rejecting — split it into a decision doc with `/decision-doc` and
cross-link the two via `related:`.
