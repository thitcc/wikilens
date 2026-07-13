---
description: Run the full local verification — frontend types + tests, Rust lint + tests; mirrors CI
allowed-tools: Bash(npx:*), Bash(npm:*), Bash(cargo:*)
---
Run WikiLens's local verification in order. Stop at the first failure and show its
output; otherwise report pass/fail per step and an overall result.

1. `npx tsc --noEmit` — frontend TypeScript type-check
2. `npm test` — frontend behavior tests (Vitest)
3. `cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings`
   — Rust compiles warning-free (clippy subsumes `cargo check`)
4. `cargo test --manifest-path src-tauri/Cargo.toml` — Rust tests pass

These are exactly the gates CI enforces (`.github/workflows/ci.yml`) — run this
before pushing a branch or opening a PR, so local-green means CI-green.

This checks code health only. For the planning vault, use `/vault-lint`.
