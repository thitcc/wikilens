---
title: Sign the Windows installers
type: plan
status: idea
created: 2026-08-05
updated: 2026-08-05
tags: [build, security]
related:
  - "[[2026-08-04_tag-triggered-release-ci]]"
  - "[[2026-08-04_auto-updater]]"
commit:
---

# Sign the Windows installers

## In simple terms
Windows treats software from an unknown publisher as suspicious. A code-signing
certificate is the thing that puts a real name on the installer instead of
"Unknown publisher" — it doesn't make the app safer, it makes Windows stop
treating it as an unknown.

## Context / problem
`release.yml` builds unsigned NSIS and MSI installers. Two consequences follow,
and they get conflated constantly:

- **Direct download.** SmartScreen runs a reputation check on downloaded
  executables and the signatures on them; with no signature and no reputation,
  the file is "marked as a higher risk" and the user gets a warning
  ([Microsoft Defender SmartScreen overview](https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/)).
  That warning is what stops a casual installer.
- **Storefront** (the current distribution plan, see
  [[2026-08-04_auto-updater]]). The reputation prompt keys on the internet
  Mark-of-the-Web, which browsers attach and a storefront client does not put
  on the files it delivers — the reason unsigned indie games don't produce
  "Windows protected your PC". **Verify on-device before relying on it**; the
  vault's rule for behavior nobody has observed here (cf. the hotkey
  virtual-key note in [[2026-07-19_hotkey-config]]).

What does not go away either way is antivirus heuristics. WikiLens registers a
global hotkey and captures regions of the screen — the same behavioral
signature as a keylogger and a screen-scraper. Unsigned plus that profile is
the classic false-positive recipe, and a Defender alert on first launch is a
worse first impression than a SmartScreen click-through.

Signing is currently a non-goal in [[2026-08-04_tag-triggered-release-ci]]
("artifacts stay unsigned, same as local builds"), which is correct for that
plan's scope — this doc is where the work itself lives.

## Goal / non-goals
- **Goal:** the tag-triggered build emits signed installers, with the signing
  credential held as CI secrets, so a downloaded WikiLens installer names a
  verifiable publisher.
- **Non-goal:** promising that signing removes the SmartScreen prompt outright.
  Reputation accrues; an OV certificate starts with none. Claims about EV
  certificates granting instant reputation are not worth planning around
  without checking what is actually true at purchase time.
- **Non-goal:** macOS/Linux notarization — Windows is the only shipping
  platform.
- **Non-goal:** eliminating AV false positives. Signing reduces them;
  submitting the binary to vendors for review is a separate lever.

## Approach
1. **The certificate is the whole project.** Since the 2023 CA/Browser Forum
   rule change, code-signing private keys must live on hardware tokens or an
   HSM — "drop a `.pfx` into a GitHub secret" is not available for a newly
   issued certificate. CI signing therefore needs either a cloud-HSM signing
   service the workflow can call, or a self-hosted runner with the token
   attached. Price that before anything else; it is the part that decides
   whether this is worth doing.
2. Identity verification: OV/EV issuance verifies a legal entity. Confirm what
   an individual developer (vs. a registered company) can obtain, and in which
   jurisdiction, before assuming a purchase path exists.
3. Wiring is the small part: Tauri's Windows bundler accepts a signing command
   / certificate thumbprint — check the exact configuration keys against the
   Tauri v2 docs at implementation time rather than trusting this line, and
   pass the credentials from repo secrets in `release.yml`.
4. Verify the output with `signtool verify /pa /v` on the built installers, and
   re-walk `docs/smoke-checklist.md` item 10 against a downloaded signed
   installer — the install path is exactly where a signing mistake shows up.

## Decisions & trade-offs
**Threshold: don't buy until distribution is imminent.** Certificates are
annual and validity starts at purchase, so buying early spends a year of
coverage on an app nobody outside this machine can install. The trigger is a
concrete plan to put WikiLens in front of someone else — a storefront
submission, or the first direct download handed to a stranger.

Under the storefront plan this is a polish item, not a blocker. If distribution
ever becomes direct download, it is promoted to a prerequisite, and it lands
alongside the auto-updater question that same decision revives.

## Status log
- 2026-08-05 — created as an idea while closing [[2026-08-04_auto-updater]],
  which had recorded signing as the concern that "actually gates" installation.
  That framing was overstated for a storefront and is corrected in that doc;
  the accurate version is the two-cases split above.
