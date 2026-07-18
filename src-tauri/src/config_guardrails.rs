//! Config-as-data guardrail tests: parse the shipped Tauri configuration and
//! pin the security invariants that otherwise exist only as prose in CLAUDE.md
//! §4–5 (see vault/2026-07-13_dependency-audit-lint-gates.md). The tests read
//! the real repo files at runtime, so they drift-check the actual shipped
//! configuration — slightly unusual for unit tests, deliberately so.
//!
//! Compiled only under `#[cfg(test)]` — the `mod` declaration in `lib.rs` is
//! gated, like `test_support`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::AppError;

/// The `src-tauri/` directory at compile time.
fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()))
}

/// Every `capabilities/*.json`, so future capability files are auto-covered.
fn capability_files() -> Vec<(PathBuf, Value)> {
    let dir = manifest_dir().join("capabilities");
    let files: Vec<(PathBuf, Value)> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("readable dir entry").path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .map(|p| {
            let json = read_json(&p);
            (p, json)
        })
        .collect();
    assert!(!files.is_empty(), "no capability files in {}", dir.display());
    files
}

/// A permission entry's identifier — the string itself, or the object form's
/// `identifier` field (the extended form carrying scopes).
fn permission_identifier(perm: &Value) -> &str {
    match perm {
        Value::String(s) => s,
        Value::Object(map) => map
            .get("identifier")
            .and_then(Value::as_str)
            .expect("object permission carries a string `identifier`"),
        other => panic!("unexpected permission entry shape: {other}"),
    }
}

/// The production CSP string. `devCsp` is deliberately unasserted: it is
/// dev-only, and deleting it falls back to this stricter prod `csp`, which
/// breaks `npm run tauri dev` visibly — no silent-drift risk.
fn prod_csp() -> String {
    let conf = read_json(&manifest_dir().join("tauri.conf.json"));
    conf["app"]["security"]["csp"]
        .as_str()
        .expect("app.security.csp is a string")
        .to_string()
}

fn csp_directives(csp: &str) -> BTreeMap<String, Vec<String>> {
    csp.split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|directive| {
            let mut tokens = directive.split_whitespace();
            let name = tokens.next().expect("directive has a name").to_string();
            (name, tokens.map(str::to_string).collect())
        })
        .collect()
}

fn directive_set(directives: &BTreeMap<String, Vec<String>>, name: &str) -> BTreeSet<String> {
    directives
        .get(name)
        .unwrap_or_else(|| panic!("csp has a {name} directive"))
        .iter()
        .cloned()
        .collect()
}

/// CLAUDE.md §4: "All HTTP happens in Rust. The webview has no network/http/fs
/// permissions." No capability file may grant an `http:` or `fs:` permission.
#[test]
fn no_capability_grants_http_or_fs_permissions() {
    let mut checked = 0;
    for (path, cap) in capability_files() {
        let perms = cap["permissions"]
            .as_array()
            .unwrap_or_else(|| panic!("{}: no permissions array", path.display()));
        for perm in perms {
            let id = permission_identifier(perm);
            assert!(
                !id.starts_with("http:") && !id.starts_with("fs:"),
                "{} grants forbidden webview permission: {id}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "vacuous run — no permissions were checked");
}

/// CLAUDE.md §5 gotcha: `opener:allow-open-url` needs its inline http(s) URL
/// scope, or `open_url` compiles fine and fails with `ForbiddenUrl` at runtime.
#[test]
fn opener_permission_keeps_its_url_scope() {
    let cap = read_json(&manifest_dir().join("capabilities").join("default.json"));
    let perms = cap["permissions"].as_array().expect("permissions array");
    let opener = perms
        .iter()
        .find(|p| permission_identifier(p) == "opener:allow-open-url")
        .expect("default.json grants opener:allow-open-url");
    let urls: BTreeSet<&str> = opener["allow"]
        .as_array()
        .expect("opener permission is the object form with an inline `allow` scope")
        .iter()
        .filter_map(|entry| entry["url"].as_str())
        .collect();
    assert!(urls.contains("https://*"), "missing https://* scope: {urls:?}");
    assert!(urls.contains("http://*"), "missing http://* scope: {urls:?}");
}

/// The debug window is a listen-only surface: it renders the `debug://…`
/// event stream and invokes no commands, so its capability must stay exactly
/// the event-loop minimum — strictly smaller than capture's.
#[test]
fn debug_capability_stays_listen_only() {
    let cap = read_json(&manifest_dir().join("capabilities").join("debug.json"));
    let windows: Vec<&str> = cap["windows"]
        .as_array()
        .expect("windows array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(windows, ["debug"], "debug capability must scope only the debug window");
    let perms: BTreeSet<&str> = cap["permissions"]
        .as_array()
        .expect("permissions array")
        .iter()
        .map(permission_identifier)
        .collect();
    assert_eq!(
        perms,
        BTreeSet::from(["core:default", "core:event:default"]),
        "the debug window invokes no commands — keep its capability listen-only"
    );
}

/// `default-src` is the fallback for every unlisted directive (media, frames,
/// workers, …) — it must stay exactly `'self'`.
#[test]
fn prod_csp_default_src_is_self_only() {
    let directives = csp_directives(&prod_csp());
    assert_eq!(
        directive_set(&directives, "default-src"),
        BTreeSet::from(["'self'".to_string()])
    );
}

/// The webview can reach the Tauri IPC endpoints and nothing else — all
/// wiki/LLM HTTP happens in Rust (CLAUDE.md §4).
#[test]
fn prod_csp_connect_src_is_ipc_only() {
    let directives = csp_directives(&prod_csp());
    assert_eq!(
        directive_set(&directives, "connect-src"),
        BTreeSet::from(["ipc:".to_string(), "http://ipc.localhost".to_string()])
    );
}

/// Images only from the bundle and data: URIs (the capture thumbnail crosses
/// IPC as a data-URI) — no remote image origins.
#[test]
fn prod_csp_img_src_is_self_and_data_only() {
    let directives = csp_directives(&prod_csp());
    assert_eq!(
        directive_set(&directives, "img-src"),
        BTreeSet::from(["'self'".to_string(), "data:".to_string()])
    );
}

/// No remote script origins and no eval. The effective script policy is
/// `script-src` when declared, else `default-src` — asserting the effective
/// set (rather than pinning `script-src`'s absence) survives the
/// security-equivalent addition of an explicit `script-src 'self'`.
#[test]
fn prod_csp_restricts_scripts_to_self_without_eval() {
    let csp = prod_csp();
    let directives = csp_directives(&csp);
    let effective = if directives.contains_key("script-src") {
        directive_set(&directives, "script-src")
    } else {
        directive_set(&directives, "default-src")
    };
    assert_eq!(effective, BTreeSet::from(["'self'".to_string()]));
    assert!(!csp.contains("unsafe-eval"), "prod CSP must never allow eval: {csp}");
}

/// CLAUDE.md §4: `ProviderInfo` is the only provider data crossing IPC and
/// deliberately never reports key material or which keys are configured. Pin
/// the exact serialized field set — extend the list only after confirming a
/// new field carries no key material.
#[test]
fn provider_info_serializes_exactly_the_known_fields() {
    let infos = crate::commands::list_providers();
    assert!(!infos.is_empty());
    for info in &infos {
        let value = serde_json::to_value(info).expect("ProviderInfo serializes");
        let keys: BTreeSet<&str> = value
            .as_object()
            .expect("ProviderInfo serializes to an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["id", "name", "defaultModel", "defaultModelLabel", "defaultModelVision"]),
            "new ProviderInfo IPC field — review it for key material, then update this pin"
        );
    }
}

/// If a real key is configured in this environment, its value must not appear
/// anywhere in the serialized provider list. Read-only env access — tests run
/// multi-threaded, so never mutate env here. Vacuous in CI (no keys set); the
/// field-set pin above is the structural guarantee there.
#[test]
fn provider_info_never_contains_configured_key_values() {
    let serialized =
        serde_json::to_string(&crate::commands::list_providers()).expect("serializes");
    for provider in crate::providers::PROVIDERS {
        let Ok(key) = std::env::var(provider.api_key_env) else {
            continue;
        };
        let key = key.trim();
        // Skip trivially short dummy values ("test") that could substring-match
        // by coincidence; real provider keys are long random strings.
        if key.len() < 8 {
            continue;
        }
        assert!(
            !serialized.contains(key),
            "{}: configured key value leaked into ProviderInfo JSON",
            provider.id
        );
    }
}

/// CLAUDE.md §4: keys are "never logged, never sent to the frontend". The
/// missing-key message names the env VAR so the user can fix it — and both
/// variant fields are `&'static str`, so a runtime key value structurally
/// cannot be embedded in this error.
#[test]
fn missing_api_key_error_names_the_env_var_not_a_value() {
    let msg = AppError::MissingApiKey {
        provider: "Anthropic",
        env_var: "ANTHROPIC_API_KEY",
    }
    .to_string();
    assert!(msg.contains("ANTHROPIC_API_KEY"), "must name the env var: {msg}");
    assert!(msg.contains("Anthropic"), "must name the provider: {msg}");
}

/// `AppError::Http` wraps transport errors from `send()` (llm.rs) — the one
/// error path where the failed request carried the API-key header (llm.rs
/// sends keys in headers only, never in URLs). Pin that the user-facing
/// Display never echoes request headers. Loopback port 1 is never bound, so
/// the connection is refused immediately; the client timeout backstops any
/// pathological blackhole — and the assertion holds for every error kind
/// (refused or timeout), so the test cannot flake on which one occurs.
#[tokio::test]
async fn http_error_display_never_echoes_request_headers() {
    const SENTINEL: &str = "WIKILENS-SENTINEL-NOT-A-REAL-KEY";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("client builds");
    let err = client
        .get("http://127.0.0.1:1/")
        .header("x-api-key", SENTINEL)
        .header("authorization", format!("Bearer {SENTINEL}"))
        .send()
        .await
        .expect_err("nothing serves on 127.0.0.1:1");
    let msg = AppError::Http(err).to_string();
    assert!(msg.starts_with("Network request failed:"), "variant wiring: {msg}");
    assert!(!msg.contains(SENTINEL), "reqwest error echoed a request header: {msg}");
}
