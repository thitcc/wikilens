---
title: PR-based delivery with a CI gate — human merges, merge commits only
type: decision
status: done
created: 2026-07-13
updated: 2026-07-13
tags: [build, testing]
related: ["[[2026-07-13_ci-pipeline-github-actions]]", "[[2026-07-13_testing-audit]]"]
commit: b80df5d
---

# PR-based delivery with a CI gate — human merges, merge commits only

## Context
Until now the repo mixed direct-to-main commits with occasional PRs (PR #1 for
the retrieval rewrite; docs and features otherwise landed straight on `main`).
With CI in place ([[2026-07-13_ci-pipeline-github-actions]]) there is finally a
gate worth standing behind — but it only protects `main` if changes arrive
through it. The codebase is fully AI-generated, which raises the value of a
human checkpoint before anything lands. Separately, the planning vault pins
`commit:` hashes from work branches, which constrains how PRs may be merged.

## Decision
Every change lands via a feature branch and a pull request: branch
`<type>/<slug>` off `main`, conventional commit prefixes, `/check` (which
mirrors CI exactly) plus `/vault-lint` before pushing, `gh pr create`, CI green
required — then the **human** merges with a **merge commit**. Claude never
merges. The `docs(vault): close …` commit rides in the same PR as the code it
closes. Declared standard by the user on 2026-07-13, immediately after PR #2
exercised the full loop.

## Consequences
- Good: nothing reaches `main` unverified — the clippy + test + type-check
  gates run on every PR, and a human review stands between AI-generated code
  and the default branch.
- Good: merge commits keep branch hashes reachable on `main`, so vault docs'
  `commit:` refs (set on close, inside the PR) stay valid forever.
- Good: `/check` and CI can no longer disagree — the local command was
  rewritten to run the identical gates (clippy replaced `cargo check`).
- Cost: solo-dev overhead — every change, even docs, waits on a PR round-trip
  and a manual merge; stacked PRs are needed when work builds on an unmerged
  branch (this very decision landed stacked on PR #2).
- Follow-ups: branch-protection rules on `main` (require the CI checks, block
  direct pushes) would enforce mechanically what this ADR states as convention;
  cargo-audit/npm audit join the gate via
  [[2026-07-13_dependency-audit-lint-gates]].

## Alternatives considered
- **Direct commits to `main` (status quo)** — no gate before landing; a broken
  commit is discovered after it ships. Rejected: with CI available, landing
  unchecked is pure downside.
- **Squash merges** — tidier history, but they rewrite branch commits into new
  hashes, orphaning every vault `commit:` reference (e.g. the close commit
  `bf1e73b` pointing at `c68f99a`/`4c2a05a` would dangle). Rejected outright.
- **Claude auto-merges when CI is green** — faster, but removes the human
  checkpoint that is the point of reviewing AI-generated changes. Rejected;
  the human merge is the last gate.
