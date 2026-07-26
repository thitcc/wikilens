---
title: Store panel-entered API keys as DPAPI-encrypted blobs in app-data
type: decision
status: active
created: 2026-07-26
updated: 2026-07-26
tags: [security, rust]
related: ["[[2026-07-26_default-mode-and-byo-api-keys]]"]
commit:
---

# Store panel-entered API keys as DPAPI-encrypted blobs in app-data

## Context

Custom API mode ("[[2026-07-26_default-mode-and-byo-api-keys]]") lets the user paste
per-provider API keys into the config panel. A key crosses IPC once (webview → Rust on save),
must never re-cross toward the webview, never render, never log — and has to live somewhere on
this machine between sessions. The forces: keys typed into a shipped app by non-developers
deserve encryption at rest (the old `.env` plaintext was a developer-tool bar, not a product
bar); the store must be unit-testable in CI (the Rust job runs on `windows-latest`); WikiLens
is Windows-only, so a cross-platform abstraction buys nothing; and the guardrail suite wants a
pinnable invariant — "no cleartext key material on disk".

## Decision

Panel-entered keys are stored per provider as `CryptProtectData` (Windows DPAPI, per-user
scope) ciphertexts, base64-encoded in `keys.json` in the app-data dir, behind a `KeyStore`
trait in `keys.rs`.

- File shape: `{ "version": 1, "keys": { "<provider_id>": "<base64 DPAPI ciphertext>" } }` —
  key presence (`hasKey`) is answered without decrypting; Remove drops the entry.
- Store discipline copied from `settings.rs`/`UserWikiStore`: tmp+rename atomic writes,
  corrupt file → `.bak` sideways, unreadable-at-startup → refuse mutations,
  persist-then-commit, poison-tolerant locks. Managed state created in `.setup()`.
- Implementation: `windows-sys` feature additions only (`Win32_Security_Cryptography`, plus
  `Win32_System_Memory` for `LocalFree`) — no new crate.
- A `CryptUnprotectData` failure (user-profile or machine migration, corrupted blob) is
  treated as "no key": the panel shows that provider's empty state again and the user
  re-pastes. Never surfaced as a crash or a cryptic vendor error.

## Consequences

- Good: encryption at rest bound to the Windows user, satisfying "stored locally, on this
  machine only" — another user account or a copied file on another machine cannot read the
  keys. All persistent app state stays in one place (app-data: `settings.json`, `wikis.json`,
  `keys.json`) under one backup/reset story and one store discipline. Real DPAPI round-trips
  run as plain unit tests on the `windows-latest` CI runner, and the "raw file never contains
  the pasted key" pin is directly testable. Uninstall/reset deletes the keys with the folder —
  no orphaned state elsewhere in the OS.
- Cost / bad: we own crypto plumbing (unsafe FFI around `CryptProtectData`/
  `CryptUnprotectData`, blob lifetime + `LocalFree`) and a small file format, forever. A
  backup restored to another machine or user profile yields undecryptable ghosts — keys
  silently revert to "not set" (defined and documented, but still a surprise). DPAPI's
  per-user scope means any process running as this user can decrypt the file — same class of
  protection as Credential Manager, not stronger.
- Follow-ups: the `KeyStore` trait ships with an in-memory test impl; the no-cleartext pin
  and the `keys.json` schema pin join the guardrail suite; README's Security & privacy section
  states the DPAPI scope honestly ("encrypted for your Windows user, on this machine").

## Alternatives considered

- **Windows Credential Manager via the `keyring` crate** — the idiomatic OS keystore, and the
  original recommendation. Rejected by the owner in favor of DPAPI: it adds a dependency for
  the same per-user protection class; entries live outside app-data (splitting the app's
  state story and surviving uninstall as orphans); and tests would touch a shared OS
  credential namespace instead of a tempdir file. Its genuine advantages — zero crypto code
  of our own, a user-auditable UI in Credential Manager — were judged not worth the split.
- **Plaintext JSON in app-data** — trivially simple and no worse than the `.env` era.
  Rejected: any process or backup reads the keys verbatim; the "no cleartext key material on
  disk" pin becomes impossible; below expectations for keys typed into a shipped app by
  non-developers.
