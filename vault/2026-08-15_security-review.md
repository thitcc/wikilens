---
title: Whole-project security review — API-key storage and user-information handling
type: research
status: done
created: 2026-08-15
updated: 2026-08-26
tags: [security, rust, tauri, frontend, llm]
related: ["[[2026-07-26_api-key-storage-dpapi]]", "[[2026-07-29_keys-are-not-a-mode-choice]]", "[[2026-08-04_game-auto-detection]]", "[[2026-07-13_testing-audit]]"]
commit:
---

# Whole-project security review — API-key storage and user-information handling

> **Status (2026-08-26):** a point-in-time record, audited at `fc380b6`. Of
> the Tier-2 items below, S3 (DPAPI buffer wiped before `LocalFree`), F6
> (`.env` load gated to debug builds), S6 (`history.json.bak` removed by
> Clear history — the `keys.json.bak` half stays) and G1 (the
> `global-shortcut` grant dropped; the capability sweep now also refuses
> `shell:`/`global-shortcut:` and pins the overlay's grant set) were fixed in
> the open-source prep ([[2026-08-26_open-source-release-0-2-0]]); S2, S4, S7
> and S8 stay open by decision. The threat model was written while the repo
> was private.

## In simple terms

This review audited WikiLens at commit `fc380b6` with a focus on how API keys are
stored and how user information (questions, answers, screenshots, foreground-window
data) is handled. The headline outcome: **no confirmed exploitable vulnerability was
found.** The key-handling architecture — a single write-only IPC path for key
material, per-user DPAPI encryption at rest, keys never crossing back over IPC in any
response, event, error, or debug payload — held up under adversarial verification.
What remains is a set of real but below-the-exploit-bar hardening items (memory-residue
of decrypted keys, a lingering `.bak` file that survives "Clear history", a redirect
edge for Anthropic's custom auth header, an unguarded `.env` load in release, an unused
`global-shortcut` grant) and one confirmed structural fact worth knowing: Tauri
capability files do not actually restrict which app commands a window can invoke in this
build, so the capture/debug webviews' narrow capabilities are convention, not
enforcement.

## Method

Multi-agent review run 2026-08-15: 7 code finders and 1 primary-source web researcher
fed candidates into 5 adversarial verifiers and 1 completeness critic; a synthesis pass
consolidated the verdicts, and one late finder finding (the release `.env` load) got a
dedicated adversarial adjudication. Audited HEAD is `fc380b6` on
`feat/overlay-position-modes`. The `/security-review` exclusion list applied (no DoS, no
theoretical races, no supply-chain, no memory-safety claims, env vars/CLI flags trusted
as user-set inputs, etc.). Focus: API-key storage and user-information handling.

The web researcher's two load-bearing library questions returned no usable result (the
agent lost its connection mid-run), so the verifiers resolved both **from the vendored
crate sources on the audited machine** — primary evidence, stronger than docs:

- **reqwest 0.12.28**: `remove_sensitive_headers` (`redirect.rs:239-251`, applied
  unconditionally in `on_request` at line 338) strips exactly `Authorization`, `Cookie`,
  `cookie2`, `Proxy-Authorization`, and `WWW-Authenticate` on any host-or-port change.
  Consequence: the OpenAI-compatible Bearer path (DeepSeek/OpenRouter/Default) **is**
  protected on cross-host redirects, but Anthropic's custom `x-api-key` header is **not**
  in the strip list and rides any cross-host https→https redirect chain the policy allows
  (≤5 hops).
- **tauri 2.11.5** (Cargo.lock-pinned): the ACL deny path for invokes
  (`src/webview/mod.rs:1823`) fires only when
  `plugin_command.is_some() || has_app_acl_manifest || !is_local`. WikiLens's `build.rs`
  is a bare `tauri_build::build()` and `gen/schemas/acl-manifests.json` has no
  `__app-acl__` entry, so **app (non-plugin) commands are not ACL-gated for local
  webviews**. Remote origins remain denied via the `!is_local` clause.

## Scope & threat model

Single-user Windows gaming desktop; private repo at the time of this audit
(2026-08-15 — it goes public with 0.2.0). Three trust boundaries: (a) the
WebView2 webview → Rust command surface, (b) untrusted remote content — LLM output, wiki
HTML/wikitext, a user-added wiki's self-reported siteinfo — entering the app, (c)
at-rest files in the app-data dir (`settings.json`, `keys.json`, `history.json`,
`wikis.json`). An attacker running code as the same Windows user is largely outside the
model: that attacker decrypts `keys.json` with one `CryptUnprotectData` call of their
own, which is the accepted floor DPAPI per-user scope sets.

## Tier 1 — Confirmed vulnerabilities

**None.** No candidate cleared the exploitability bar, and this is a substantive result
rather than a soft one: every seeded candidate was adversarially attacked, and the ones
that survived did so as *mechanisms* without a crossing of any trust boundary. Tiering
here is by exploitability, not verdict certainty — the Tauri app-command scoping fact
(S8) is a **confirmed** factual finding (confidence 9) yet sits in Tier 2, because all
three webviews run only bundled first-party code under a tight CSP, remote origins are
ACL-denied, and even full command access yields no key material (`set_api_key` at
`commands.rs:560` is write-only; `list_key_status` returns presence booleans). The
candidates that fell short did so for consistent reasons: either the attacker position
required (same-user code execution, memory-dump access, a compromised LLM vendor)
already trumps the asset being protected, or the payload never contains a secret and
never returns data to an attacker.

## Tier 2 — Hardening & design recommendations

**No zeroization of key material** (`src-tauri/src/keys.rs:228`). Decrypted keys live in
ordinary heap `String`s: `DpapiKeyStore::get()` returns one (keys.rs:243), `LlmTarget`
clones it per answer/rewrite target (`target.rs:130`), and `llm.rs` makes further copies
building auth headers (llm.rs:242, 273, 482, 504). Freed copies linger in heap pages
reachable by the pagefile, hibernation file, or crash dumps. No concrete exploit under
this threat model (every actor who could harvest those bytes already defeats DPAPI
trivially), but cheap to fix: wrap the decrypted key in `Zeroizing<String>` (zeroize
crate) from `unprotect()` through `LlmTarget`, and build the Bearer value into a
zeroizing buffer.

**DPAPI plaintext buffer freed without wiping** (`src-tauri/src/keys.rs:351`).
`unprotect()` copies the decrypted key out of the `CryptUnprotectData` output blob, then
`LocalFree(output.pbData)` releases the OS buffer un-wiped (keys.rs:347-354), leaving a
dead plaintext copy in the local heap. Same residue class as the zeroization item. Fix:
`std::ptr::write_bytes(output.pbData, 0, output.cbData as usize)` (or `SecureZeroMemory`)
between the copy and `LocalFree`, symmetrically in `protect()`.

**Cross-host redirects carry Anthropic's `x-api-key`** (`src-tauri/src/http.rs:148`).
Settled by the vendored-source research above: the shared client's redirect policy
(http.rs:148-161, used by every request via `state.rs:61`) refuses only https→http and
>5 hops. reqwest strips `Authorization` cross-host — protecting the Bearer providers —
but `x-api-key` is not stripped and would follow any cross-host https redirect
`api.anthropic.com` issued, up to 5 hops. Not exploitable here: keyed requests go only to
registry-hardcoded endpoints or trusted `WIKILENS_DEFAULT_*` env, so triggering it
requires a compromised vendor or a TLS break. Fix: refuse cross-host hops for requests
carrying `x-api-key` (or disable redirects entirely on the LLM client — provider APIs
never legitimately redirect a POST cross-host, so a same-host-only rule is free).

**Release build loads `.env` from the launch directory — config-injection**
(`src-tauri/src/lib.rs:41`). `dotenvy::dotenv()` runs unguarded in release, and dotenvy
walks the cwd and every ancestor (dotenvy 0.15.7 `find.rs:49-50`). In Default mode the
`WIKILENS_DEFAULT_API_URL`/key/model vars (`target.rs`) define where the user's question
and attached screenshot are sent, so a planted `.env` is a real redirect/exfil
mechanism. Adjudicated below the exploit bar (confidence 8) because it is **inert in
Custom mode** (the state of any BYO-key user); `WIKILENS_DEFAULT_*` can force Default
mode only on a **first launch with no prior `settings.json`** (lib.rs:94-103; the stored
mode wins forever after); dotenvy is **non-override** (`iter.rs:34` fills only unset
vars), so a Default user who set the vars via OS env as the docs prescribe is immune; and
the one required capability — same-user write to the launch cwd/ancestor — is already
code-execution-equivalent on Windows. Note the installer defaults to NSIS
`currentUser`, so even the exe dir is user-writable — the "Program Files, admin-only"
defense does not apply. Fix-pointer: gate the call as
`#[cfg(debug_assertions)] dotenvy::dotenv().ok();` so release builds ignore `.env`
entirely (packaged installs already use OS env vars per README/CLAUDE.md, so no
functionality is lost); optionally load only from a fixed trusted path.

**Self-reported siteinfo `server` controls the persisted endpoint**
(`src-tauri/src/wiki/probe.rs:122`). `derive_endpoints` builds `api_url`/`page_url` from
the probed wiki's own siteinfo `server` string, not from what the user typed;
`normalize_base_url` (probe.rs:56) applies no loopback/RFC1918 block (deliberate — LAN
wikis are supported). A hostile wiki the user adds can pin future requests to a different
host than the user saw, including an absolute `http://…` value that silently downgrades
protocol — the one path *not* covered by the redirect policy's https→http refusal. Blind,
credential-less, GET-only, no read-back channel, victim must deliberately add the wiki —
inside the SSRF exclusion class. Optional hardening: reject a siteinfo `server` whose host
differs from the probed host, and refuse the https→http downgrade in derivation the same
way the redirect policy does.

**`history.json.bak` survives "Clear history"** (`src-tauri/src/history.rs:92`). A corrupt
`history.json` at startup is renamed sideways to `.bak` and no code path ever deletes it;
`clear()` (history.rs:161) rewrites only the live file. After a clear, the `.bak` is the
sole surviving copy of Q&A the user believes deleted — indefinitely. The same
rename-sideways pattern in `DpapiKeyStore::load` (keys.rs:128) can retain a removed key's
DPAPI ciphertext (mitigated by per-user encryption). Fix: delete/rotate the `.bak` in
`clear()` and on key removal; sweep the crash-window `.json.tmp` files into the same
cleanup.

**App commands are not per-window scoped** (`src-tauri/build.rs:2`). Confirmed mechanism
(see Method and the Resolved section below). Fix-pointer if enforcement is wanted: enable
the app ACL via `tauri_build::Attributes::new().app_manifest(...)` with per-command
permission files — noting this flips enforcement on globally, so `default.json` must then
enumerate all 27 overlay commands and `capture.json` must add
`finish_capture`/`cancel_capture` or those windows break; `config_guardrails.rs` can then
pin the manifest's existence.

**Unused `global-shortcut:allow-register` grant; capability sweep is a prefix filter**
(`src-tauri/capabilities/default.json:15`; critic-flagged, verified present). All hotkey
registration happens Rust-side (`hotkey.rs`); no frontend code imports the global-shortcut
plugin (grep-confirmed), yet the overlay webview holds a live ACL grant to register
OS-global shortcuts directly — the exact key-swallowing failure class the Shift+C history
documents. Several `core:window:*` grants in the same file are similarly unconsumed by
frontend code. The `no_capability_grants_http_or_fs_permissions` sweep in
`config_guardrails.rs` filters only the literal `http:`/`fs:` prefixes and is blind to
these (and to a future `shell:` grant). Unlike the app-command scoping item, these are
*plugin/core* permissions the ACL does govern. Fix: remove the unused grants; convert the
sweep to an allowlist.

**Settled by refutation, one line each.** *Default-mode key in process env* is a trusted
user-set input by the audit's own rules, and DPAPI adds nothing against a same-user
process anyway; residual is the README's existing packaged-app posture (OS env vars, no
`.env`), no code change. *The capture full-PNG fallback* at `capture.rs:92` sends the
*cropped, ≤1568px* attachment, never the full monitor snapshot (which stays as a raw
`RgbaImage` in `PendingShot`), and the branch requires the `image` crate to fail decoding
its own just-encoded PNG; optional tidiness only (a 1×1 placeholder instead of
`unwrap_or(&self.png)`).

**Noted-but-excluded observation.** `wiki/html.rs`'s `close_tag` pops through dropped
subtrees on a mismatched close tag, so `<script>x</em>leaked</script>` emits "leaked" into
the LLM context — a real parser differential, but it grants a hostile wiki no capability
it lacks (CSS-hidden content already passes the reducer by design; wiki-content prompt
injection is inherent to RAG and excluded).

## Tier 3 — Affirmed invariants

These held under active adversarial tracing, not just prose review, and are the reason to
trust this codebase:

- **Key material crosses IPC exactly once**, webview → Rust via `set_api_key`
  (`commands.rs:560`), and never back: no response, event, error string, or debug payload
  carries it on any branch. `list_key_status` returns booleans. The guardrail sentinel
  tests exercise the real `DpapiKeyStore`, so the pin is live.
- **No key-holding struct derives `Debug`/`Serialize`/`Clone`**: `LlmTarget`
  (`target.rs:15`) and `AskTargets` (`target.rs:56`) have no derives at all — an
  accidental `{:?}` of a key would not compile. `keys.json` holds only base64 DPAPI
  ciphertexts, pinned by `raw_file_never_contains_the_key`.
- **All error legs are key-free**: keys.rs, target.rs resolver, and DPAPI diagnostics
  carry only OS error text, provider ids, or friendly copy — sentinel-tested to never echo
  values (`error_copy_never_echoes_values`, `legacy_env_notices`).
- **CSP is tight** (`tauri.conf.json:46`): `default-src 'self'; connect-src ipc:
  http://ipc.localhost; img-src 'self' data:` — no network egress from any webview, no
  zero-click render-time exfil for prompt-injected image URLs.
- **No HTML injection sink anywhere in `src/`**: zero matches for
  `dangerouslySetInnerHTML`/`innerHTML`/`document.write`; `AnswerView` uses react-markdown
  without `rehype-raw`; the opener scope refuses `javascript:`/`data:`/`file:` at the
  capability layer.
- **`detect::reduce` privacy boundary holds** (`reduce_never_emits_a_path`): only exe
  basename + parent leaf exist Rust-side, and only the matched static game id ever crosses
  IPC or reaches stderr.
- **Debug payloads structurally cannot carry keys**: fixed pin-tested key sets, `outcome`
  as `&'static str` (`debug.rs:109,146`); all `debug://` events route through the
  `emit_to(debug)` sink.
- **The capability sweep holds for what it claims** — no `http:`/`fs:` grants exist
  anywhere — with the caveat above that it is a prefix filter, not an allowlist. The
  capture/debug capability pins constrain the JSON and the pages' own code, not runtime
  app-command access (see below).
- **At-rest stores share one crash-safe pattern** (write `.tmp`, rename over target),
  refuse mutations on unreadable files, and capture PNGs/thumbnails never touch disk. The
  SSE parsers fail closed and are proptest-pinned panic-free; the 1 MiB stream cap counts
  post-gzip bytes.

## Resolved structural question

**Do Tauri capabilities scope app `invoke_handler` commands per-window? No — not in this
build.** Verified against the vendored tauri 2.11.5 source: the ACL check for application
(non-plugin) commands is skipped for local origins unless the app ships an app ACL
manifest (`src/webview/mod.rs:1823`, gate
`plugin_command.is_some() || has_app_acl_manifest || !is_local`). WikiLens's `build.rs` is
a bare `tauri_build::build()`, and the generated `gen/schemas/acl-manifests.json` contains
no `__app-acl__` entry — confirmed empirically by the overlay successfully invoking app
commands with a capability that lists none of them.

**Consequence:** the capture and debug webviews can invoke every one of the 27 commands in
`lib.rs`'s `invoke_handler` — `ask`, `set_api_key`, `list_history`, `set_hotkey`, and the
rest — despite `capture.json`/`debug.json` granting only window/event permissions. The
capability descriptions ("invokes no app commands") and the
`debug_capability_grants_only_events_and_drag` test pin what those pages' *own code* does,
not what the runtime *allows*. Plugin and core permissions (window ops, events, opener,
global-shortcut) **are** enforced per-window, and remote origins are denied outright — so
the flatness is a local-webview, defense-in-depth gap only, contained by the CSP and the
write-only key surface.

## Appendix — findings ledger

Verifier confidences are on a 0–10 scale; the critic's row uses a 0–1 scale. Refuted rows
are retained deliberately so the reader sees what was checked and dismissed.

| id | title | verdict | tier | conf. | file:line |
|---|---|---|---|---|---|
| S1 | Default-mode key plaintext in env (no DPAPI) | refuted | 3 | 9 | src-tauri/src/target.rs:65 |
| S2 | No zeroization of key material | partial | 2 | 8 | src-tauri/src/keys.rs:228 |
| S3 | `unprotect()` LocalFrees plaintext un-wiped | fixed 2026-08-26 | 2 | 8 | src-tauri/src/keys.rs:351 |
| S4 | Cross-host redirect carries `x-api-key` | partial | 2 | 8 | src-tauri/src/http.rs:148 |
| F6 | Release `.env` load → config-injection redirect | fixed 2026-08-26 | 2 | 8 | src-tauri/src/lib.rs:41 |
| S5 | Full-PNG-over-IPC fallback on thumb failure | refuted | 3 | 9 | src-tauri/src/capture.rs:92 |
| S6 | `history.json.bak` survives Clear history | fixed 2026-08-26 (history; `keys.json.bak` open) | 2 | 8 | src-tauri/src/history.rs:92 |
| S7 | siteinfo `server` controls persisted endpoint + http downgrade | partial | 2 | 8 | src-tauri/src/wiki/probe.rs:122 |
| S8 | No per-window scoping of app commands (no app ACL manifest) | confirmed | 2 | 9 | src-tauri/build.rs:2 |
| G1 | Unused `global-shortcut` grant; sweep is a prefix filter | fixed 2026-08-26 | 2 | 0.6 | src-tauri/capabilities/default.json:15 |

### Files reviewed

Rust: `keys.rs`, `target.rs`, `commands.rs`, `llm.rs`, `models.rs`, `http.rs`, `lib.rs`,
`state.rs`, `history.rs`, `capture.rs`, `window.rs`, `hotkey.rs`, `tray.rs`, `debug.rs`,
`debug_window.rs`, `detect/*`, `wiki/{probe,user,html,wikitext,search,titles,fetch}.rs`,
`config_guardrails.rs`, `build.rs`. Config: `tauri.conf.json`, `capabilities/*.json`,
`.github/workflows/*`. Frontend: `App.tsx`, `api.ts`, `AnswerView.tsx`, `SourceList.tsx`,
`SettingsMenu.tsx`, `HistoryMenu.tsx`, `capture/main.ts`, `debug/*`, localStorage
readers. Vendored crates: `reqwest 0.12.28` (redirect.rs), `tauri 2.11.5`
(webview/mod.rs), `dotenvy 0.15.7` (find.rs, iter.rs).
