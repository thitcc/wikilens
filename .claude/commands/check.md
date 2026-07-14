---
description: Run the full local verification — frontend types + tests, Rust lint + tests; mirrors CI's code gates
allowed-tools: Bash(npx:*), Bash(npm:*), Bash(cargo:*)
---
Run WikiLens's local verification in order. Stop at the first failure and show its
output; otherwise report pass/fail per step and an overall result.

1. `npx tsc --noEmit` — frontend TypeScript type-check
2. `npm test` — frontend behavior tests (Vitest)
3. `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings`
   — Rust compiles warning-free (clippy subsumes `cargo check`)
4. `cargo test --manifest-path src-tauri/Cargo.toml` — Rust tests pass

These are the code gates CI enforces (`.github/workflows/ci.yml`) — run this
before pushing a branch or opening a PR, so local-green means CI-green for
everything your code changes. CI additionally runs dependency audits
(`npm audit --audit-level=high`, `cargo audit`); those stay CI-side because
they depend on the network and the advisory databases — a fresh advisory can
turn CI red with no code change.

This checks code health only. For the planning vault, use `/vault-lint`.
