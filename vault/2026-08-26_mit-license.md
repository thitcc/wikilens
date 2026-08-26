---
title: The repo is MIT-licensed, with the wiki fixtures carved out by a root notice
type: decision
status: done
created: 2026-08-26
updated: 2026-08-26
tags: [build, security]
related:
  - "[[2026-08-26_open-source-release-0-2-0]]"
  - "[[2026-07-13_parser-snapshot-property-tests]]"
commit: 9145706
---

# The repo is MIT-licensed, with the wiki fixtures carved out by a root notice

## Context
Going public needs a license, and the choice is constrained by two things.
The dependency tree: a full sweep of the 602 crates in `Cargo.lock` and the
218 npm packages in `node_modules` found only permissive and weak-copyleft
licenses (MIT / Apache-2.0 duals, MIT, Apache-2.0, Zlib, Unicode-3.0, ISC,
BSD, five MPL-2.0 crates, two `r-efi` crates offering an LGPL branch beside
MIT) — nothing that forces a copyleft outcome, so MIT, Apache-2.0, a dual,
or MPL-2.0 would all be compatible. And the tracked wiki content: four test
fixtures under `src-tauri/src/wiki/fixtures/` plus their four insta
snapshots are verbatim MediaWiki pages captured 2026-07-13 with per-file
provenance headers — CC BY-SA 3.0 (core-keeper.fandom.com), CC BY-SA 2.5
(en.uesp.net) and, for the two Stardew samples, **CC BY-NC-SA 3.0**
(stardewvalleywiki.com). Those bytes are `#[cfg(test)]`-only and never reach
the shipped binary, but the repo redistributes them, and a blanket license
would misstate their terms — NonCommercial cannot be granted under any OSI
license. Runtime wiki fetches are display with source links, not
redistribution, and carry no obligation.

## Decision
The repository is licensed under MIT (copyright Thiago Tenório), and a
root `THIRD-PARTY-NOTICES.md` excludes the eight fixture/snapshot files
from that grant, listing each with its source URL, capture date and CC
license — the NonCommercial ones flagged explicitly.

## Consequences
- Good: one short license file a stranger recognises; `Cargo.toml`,
  `package.json` and `tauri.conf.json` all carry the same SPDX id; the
  fixtures keep exercising the paths they were captured for (the Stardew
  page is the only non-Fandom default-engine infobox sample in the tree).
- Cost / bad: the NC bytes remain in every historical revision — the
  notice says so; anyone repackaging the repo commercially must drop those
  eight files. No patent grant (Apache-2.0's one distinguishing feature).
- Follow-ups: the README's License section points at both files; the
  `bundle.license` field names MIT so the installers carry it (no
  `licenseFile` — that would add an EULA page to both installers).

## Alternatives considered
- **Apache-2.0** — explicit patent grant and NOTICE mechanics; more text
  for a desktop utility with no patent exposure. Rejected as ceremony.
- **MIT OR Apache-2.0 dual** — the Rust-crate convention, aimed at
  libraries consumed by other projects; an application gains nothing from
  two license files. Rejected.
- **Regenerate the two Stardew fixtures from a BY-SA wiki before flipping
  public** — the audit's two verifiers split on this. Regenerating removes
  the NC term from HEAD but not from history, costs a recapture plus two
  reviewed snapshot diffs, and loses the one default-engine (non-Fandom)
  infobox sample the HTML reducer's snapshot pins. The notice carve-out is
  the standard treatment for verbatim third-party test data and is
  sufficient for files that never ship. Rejected for now; revisit if the
  NC term ever matters (e.g. a storefront listing that audits the repo).
- **Relicense the repo CC BY-NC-SA** — not an open-source license.
  Rejected outright.
