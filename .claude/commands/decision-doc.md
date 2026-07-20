---
description: Scaffold a new planning-vault decision (ADR) doc from vault/templates/decision.md
argument-hint: <short description or kebab-slug>
allowed-tools: Read, Write, Edit
---
Create a new decision doc (MADR-lite ADR) in the WikiLens planning vault from
`vault/templates/decision.md`. Use this only for a **real fork** — a choice with an
alternative we're deliberately rejecting. If there's no rejected alternative, it's a
plan, not a decision — use `/plan-doc` instead.

Steps:
1. Derive a **kebab-case slug** (for the filename) and a **human-sentence title**
   from "$ARGUMENTS" and what we've just been discussing. If "$ARGUMENTS" is empty,
   derive both from the recent discussion — don't stop to ask.
2. Filename = `vault/<today>_<slug>.md`, where `<today>` is today's date as ISO
   `YYYY-MM-DD`. If that file already exists, **STOP and report it** — filenames are
   permanent IDs; never overwrite or rename an existing doc.
3. Copy the template body and write **clean** frontmatter (drop the template's
   teaching `#` comments — real docs carry values only):
   - `title`: the human sentence (also the `#` H1)
   - `type: decision`
   - `status: active`
   - `created` / `updated`: today's date
   - `tags`: only tags already in the registry in `vault/index.md` — never invent
     one; leave `[]` if none fit
   - `related`: quoted wikilinks — if this decision came out of a plan, set it to that
     plan's wikilink (and add the back-link on the plan). Else `[]`.
   - `commit`: leave empty — set it on close, once the code is committed. Branch
     hashes are fine: PRs merge via merge commit (never squash), so they stay
     valid on `main`.
4. Fill **Context / Decision / Consequences / Alternatives considered** from what we
   discussed. The **Alternatives considered** section is the point of a decision doc:
   name what we rejected and why.
5. Report the created path and its frontmatter. There is no index list to update —
   `vault/index.md`'s dashboards derive themselves from frontmatter.
