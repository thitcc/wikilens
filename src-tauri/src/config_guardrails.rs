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

/// The debug page renders the `debug://…` event stream and invokes no app
/// commands; its only command IPC is the core `start_dragging` the
/// `data-tauri-drag-region` attribute fires (the window is undecorated and
/// draggable). Pin exactly that — events + drag, nothing more.
#[test]
fn debug_capability_grants_only_events_and_drag() {
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
        BTreeSet::from(["core:default", "core:event:default", "core:window:allow-start-dragging"]),
        "the debug page invokes no app commands — events + the drag-region's start_dragging only"
    );
}

/// Boot-race guardrail: every configured window defers creation to `.setup()`
/// (`"create": false`). Tauri builds `create: true` config windows BEFORE the
/// setup hook runs, so such a webview's boot invokes race setup's `.manage()`
/// calls — a packaged build loads its bundled assets fast enough to win,
/// failing commands with "state not managed" (dev's slower Vite loads masked
/// it; vault/2026-08-02_packaged-boot-state-race.md). lib.rs builds these
/// windows in setup after the stores are managed; a new `create: true` window
/// would silently reopen the race.
#[test]
fn config_windows_defer_creation_to_setup() {
    let conf = read_json(&manifest_dir().join("tauri.conf.json"));
    let windows = conf["app"]["windows"].as_array().expect("app.windows array");
    assert!(!windows.is_empty(), "vacuous run — no windows in tauri.conf.json");
    for win in windows {
        let label = win["label"].as_str().expect("window has a label");
        assert_eq!(
            win["create"],
            Value::Bool(false),
            "window '{label}' must set \"create\": false and be built in lib.rs setup \
             after the stores are managed — config windows are created before setup runs"
        );
    }
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

/// CLAUDE.md §4: `ProviderInfo` never carries key material — key *presence*
/// crosses separately as `KeyStatus`. Pin the exact serialized field set —
/// extend the list only after confirming a new field carries no key material.
/// Every registry provider is keyed here: `provider_infos` filters to keyed
/// providers, and an empty list would make this pin vacuous.
#[test]
fn provider_info_serializes_exactly_the_known_fields() {
    use crate::keys::KeyStore;
    let keys = crate::keys::InMemoryKeyStore::default();
    for provider in crate::providers::PROVIDERS {
        keys.set(provider.id, "pin-test-key").expect("seed");
    }
    let infos = crate::commands::provider_infos(&keys);
    assert_eq!(infos.len(), crate::providers::PROVIDERS.len());
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

/// `KeyStatus` is the one IPC type that reports key state. Pin the exact
/// serialized field set — presence booleans may cross, key material may not;
/// extend the list only after confirming a new field carries none.
#[test]
fn key_status_serializes_exactly_the_known_fields() {
    let rows = crate::commands::key_status(&crate::keys::InMemoryKeyStore::default());
    assert!(!rows.is_empty());
    for row in &rows {
        let value = serde_json::to_value(row).expect("KeyStatus serializes");
        let keys: BTreeSet<&str> = value
            .as_object()
            .expect("KeyStatus serializes to an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["id", "name", "hasKey"]),
            "new KeyStatus IPC field — review it for key material, then update this pin"
        );
    }
}

/// A stored key's value must never appear in any serialized IPC surface that
/// touches provider/key state. Seeds the real DPAPI store (the exact type the
/// shipping commands serialize from) with a sentinel, then checks the two
/// lists. Since `provider_infos` filters to keyed providers, the
/// sentinel-keyed provider actually appears in the second list — the value
/// check is live, not merely structural.
#[test]
fn key_status_never_contains_stored_key_values() {
    use crate::keys::KeyStore;
    const SENTINEL: &str = "WIKILENS-SENTINEL-NOT-A-REAL-KEY";
    let dir = tempfile::tempdir().expect("tempdir");
    let store = crate::keys::DpapiKeyStore::load(dir.path().join("keys.json"));
    store.set("anthropic", SENTINEL).expect("store the sentinel");

    let statuses =
        serde_json::to_string(&crate::commands::key_status(&store)).expect("serializes");
    assert!(statuses.contains("\"hasKey\":true"), "vacuous run: {statuses}");
    assert!(!statuses.contains(SENTINEL), "key value leaked into KeyStatus JSON");

    let providers =
        serde_json::to_string(&crate::commands::provider_infos(&store)).expect("serializes");
    assert!(providers.contains("\"id\":\"anthropic\""), "vacuous run: {providers}");
    assert!(!providers.contains(SENTINEL), "key value leaked into ProviderInfo JSON");
}

/// `SettingsInfo` is the settings envelope crossing IPC. Pin its exact field
/// sets (top level + the nested `defaultMode`), and that a never-chosen mode
/// crosses as an explicit null rather than an absent field.
#[test]
fn settings_info_serializes_exactly_the_known_fields() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = crate::settings::SettingsStore::load(dir.path().join("settings.json"));
    let info = crate::commands::settings_info(
        &store,
        crate::commands::DefaultModeInfo {
            configured: false,
            vision: false,
        },
    );
    let value = serde_json::to_value(&info).expect("SettingsInfo serializes");
    let top: BTreeSet<&str> = value
        .as_object()
        .expect("SettingsInfo serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        BTreeSet::from(["hotkeys", "mode", "defaultMode", "position"]),
        "new SettingsInfo IPC field — review it for key material, then update this pin"
    );
    assert!(value["mode"].is_null(), "never-chosen mode must cross as null");
    let nested: BTreeSet<&str> = value["defaultMode"]
        .as_object()
        .expect("defaultMode serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(nested, BTreeSet::from(["configured", "vision"]));
    // `position` doubles as the `settings://position` event payload
    // (`PositionInfo`). The stored manual *coordinates* are the sensitive bit
    // — they say where on screen the player keeps the panel — and must stay
    // Rust-side: mode, anchor, and the padlock only.
    let position: BTreeSet<&str> = value["position"]
        .as_object()
        .expect("position serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        position,
        BTreeSet::from(["mode", "anchor", "locked", "manualEdge"]),
        "new PositionInfo IPC field — coordinates must never cross; update this pin deliberately"
    );
    assert_eq!(value["position"]["mode"], "anchored");
    assert_eq!(value["position"]["anchor"], "top-right");
    assert_eq!(value["position"]["locked"], false);
    // The edge is the ONLY Manual fact that crosses — never x/y.
    assert!(value["position"]["manualEdge"].is_null());
}

/// `overlay://shown` became a payload-carrying IPC surface when foreground
/// game detection landed. The probe reads a full executable path — which
/// embeds the user's install layout and often their username — and a window
/// caption, which can carry server addresses and save names. Neither may ever
/// cross: the only thing allowed out is a resolved game id from the shipped
/// rules table. Pin the field set, and that "nothing detected" crosses as an
/// explicit null rather than an absent field.
#[test]
fn shown_info_serializes_exactly_the_known_fields() {
    let nothing = serde_json::to_value(crate::window::ShownInfo { detected_game: None })
        .expect("ShownInfo serializes");
    let keys: BTreeSet<&str> = nothing
        .as_object()
        .expect("ShownInfo serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["detectedGame"]),
        "new ShownInfo IPC field — review it for path and caption material, then update this pin"
    );
    assert!(
        nothing["detectedGame"].is_null(),
        "no detection must cross as null"
    );

    // And a detection crosses as the bare registry id, nothing richer.
    let detected = serde_json::to_value(crate::window::ShownInfo {
        detected_game: Some("grounded".to_string()),
    })
    .expect("ShownInfo serializes");
    assert_eq!(detected["detectedGame"], serde_json::json!("grounded"));
}

/// `HistoryEntry` is persisted to disk and crosses IPC (`list_history`). Pin
/// its exact serialized field sets (top level + the nested sources) — answer
/// text is LLM output and belongs; wiki page text and key material do not.
/// Extend the lists only after the same review as the other IPC pins.
#[test]
fn history_entry_serializes_exactly_the_known_fields() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = crate::history::HistoryStore::load(dir.path().join("history.json"));
    store
        .append(crate::history::NewEntry {
            game_id: "stardew-valley".to_string(),
            game_name: "Stardew Valley".to_string(),
            question: "best crops for winter".to_string(),
            answer: "Winter crops are limited — see **Winter Seeds**.".to_string(),
            sources: vec![crate::commands::Source {
                title: "Winter".to_string(),
                url: "https://stardewvalleywiki.com/Winter".to_string(),
            }],
            provider_name: "Anthropic".to_string(),
            model: Some("claude-sonnet-5".to_string()),
            had_image: false,
        })
        .expect("append");
    let entries = store.list();
    let value = serde_json::to_value(&entries[0]).expect("HistoryEntry serializes");
    let top: BTreeSet<&str> = value
        .as_object()
        .expect("HistoryEntry serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top,
        BTreeSet::from([
            "id",
            "createdMs",
            "gameId",
            "gameName",
            "question",
            "answer",
            "sources",
            "providerName",
            "model",
            "hadImage",
        ]),
        "new HistoryEntry field — review it (no wiki page text, no key material), then update this pin"
    );
    let source: BTreeSet<&str> = value["sources"][0]
        .as_object()
        .expect("Source serializes to an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(source, BTreeSet::from(["title", "url"]));
}

/// CLAUDE.md §4: keys are "never logged, never sent to the frontend". The
/// missing-key message points at the Settings panel and names the fix (add a
/// key) rather than a section heading — headings move, the fix doesn't — and
/// the variant's one field is `&'static str`, so a runtime key value
/// structurally cannot be embedded in this error.
#[test]
fn missing_api_key_error_points_at_settings_not_a_value() {
    let msg = AppError::MissingApiKey {
        provider: "Anthropic",
    }
    .to_string();
    assert!(msg.contains("Anthropic"), "must name the provider: {msg}");
    assert!(msg.contains("Settings"), "must point at the panel: {msg}");
    assert!(msg.contains("add one"), "must name the fix: {msg}");
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

// ── Retrieval eval fixture (eval/questions.json) ─────────────────────────────
//
// The Node eval harness mirrors this crate's retrieval phase and reads its
// wikis from a JSON fixture. Pin the fixture to the real registry so a game
// endpoint change (api_url, page_url, namespace) can't silently leave the
// eval measuring a wiki the app no longer talks to — and keep the fixture's
// own vocabularies closed (`eval/README.md`).

fn eval_fixture() -> Value {
    read_json(&manifest_dir().join("..").join("eval").join("questions.json"))
}

#[test]
fn eval_fixture_builtin_wikis_match_the_game_registry() {
    let fixture = eval_fixture();
    let wikis = fixture["wikis"].as_object().expect("fixture.wikis is an object");
    assert!(!wikis.is_empty(), "fixture lists no wikis");
    for (key, w) in wikis {
        assert_eq!(w["id"].as_str(), Some(key.as_str()), "wiki key {key} must equal its id");
        if w["builtin"] != Value::Bool(true) {
            continue; // user-added wiki (wikis.json shape) — no registry twin
        }
        let game = crate::wiki::games::find_game(key)
            .unwrap_or_else(|| panic!("fixture wiki {key} is flagged builtin but not registered"));
        assert_eq!(w["api_url"].as_str(), Some(game.api_url.as_str()), "{key}: api_url drifted");
        assert_eq!(w["page_url"].as_str(), Some(game.page_url.as_str()), "{key}: page_url drifted");
        let ns = w["search_namespace"].as_str().map(str::to_string);
        assert_eq!(ns, game.search_namespace, "{key}: search_namespace drifted");
        assert_eq!(w["name"].as_str(), Some(game.name.as_str()), "{key}: name drifted");
    }
}

#[test]
fn eval_fixture_questions_are_well_formed() {
    const STYLES: &[&str] = &["entity", "stat", "howto", "negation", "typo", "control", "compare"];
    const SOURCES: &[&str] = &["history", "hand", "synthetic"];
    let fixture = eval_fixture();
    let wikis = fixture["wikis"].as_object().expect("fixture.wikis is an object");
    let questions = fixture["questions"].as_array().expect("fixture.questions is an array");
    assert!(questions.len() >= 40, "fixture shrank below the original 40 questions");
    let mut ids = BTreeSet::new();
    for q in questions {
        let id = q["id"].as_str().expect("question id is a string");
        assert!(ids.insert(id.to_string()), "duplicate question id {id}");
        let game = q["game"].as_str().expect("question game is a string");
        assert!(wikis.contains_key(game), "{id}: unknown wiki {game}");
        let style = q["style"].as_str().unwrap_or("");
        assert!(STYLES.contains(&style), "{id}: style {style:?} outside the closed vocabulary");
        let source = q["source"].as_str().unwrap_or("");
        assert!(SOURCES.contains(&source), "{id}: source {source:?} outside the closed vocabulary");
        assert!(!q["question"].as_str().unwrap_or("").trim().is_empty(), "{id}: empty question");
        let gold = q["gold"].as_array().unwrap_or_else(|| panic!("{id}: gold is an array"));
        assert!(!gold.is_empty(), "{id}: no gold title");
        if let Some(fact) = q.get("fact") {
            assert!(fact["expected"].is_string() && fact["evidence"].is_string(), "{id}: fact needs expected + evidence");
        }
    }
}
