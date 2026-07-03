---
description: Run the full local verification — frontend types, Rust compile, Rust tests
allowed-tools: Bash(npx:*), Bash(cargo:*)
---
Run WikiLens's local verification in order. Stop at the first failure and show its
output; otherwise report pass/fail per step and an overall result.

1. `npx tsc --noEmit` — frontend TypeScript type-check
2. `cargo check --manifest-path src-tauri/Cargo.toml` — Rust compiles
3. `cargo test --manifest-path src-tauri/Cargo.toml` — Rust tests pass

This checks code health only. For the planning vault, use `/vault-lint`.
