//! Resolved LLM call targets — the seam between "where the model and
//! credential come from" (registry + `KeyStore` in Custom mode; the
//! `WIKILENS_DEFAULT_*` env family in Default mode) and the wire client
//! (`llm.rs`). The Default resolver is the swappable boundary a hosted tier
//! replaces later: nothing downstream of an [`LlmTarget`] knows or cares
//! which mode produced it (vault/2026-07-26_default-mode-and-byo-api-keys.md).

use crate::providers::{Provider, ProviderKind};

/// Everything one LLM call needs. Runtime strings for endpoint/key/model;
/// `name`/`debug_id` stay `&'static str` because `AppError::Llm.provider` is
/// pinned `&'static str` (config_guardrails — a runtime key value structurally
/// can't be embedded) and both sources are static: registry entries, or the
/// `DEFAULT_TARGET_*` consts below.
pub struct LlmTarget {
    pub kind: ProviderKind,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    /// User-facing provider label for error copy ("Default" in Default mode —
    /// the vendor never surfaces there).
    pub name: &'static str,
    /// Debug-table/window id — feeds the "provider / model" pair strings
    /// whose equality suppresses the rewrite row (debug.rs). Identical
    /// targets must carry identical ids.
    pub debug_id: &'static str,
    /// Kept `'static` (registry slices or `&[]`). The Anthropic request
    /// builders deliberately ignore these — llm.rs preserves that asymmetry.
    pub extra_headers: &'static [(&'static str, &'static str)],
}

/// The footer chip's word, and the only provider label Default mode ever
/// shows. Renaming the tier = changing these two consts (+ the frontend
/// `DEFAULT_MODE_LABEL`).
pub const DEFAULT_TARGET_NAME: &str = "Default";
pub const DEFAULT_TARGET_DEBUG_ID: &str = "default";

impl LlmTarget {
    /// Custom-mode constructor: a registry provider + its stored key + the
    /// resolved model.
    pub fn from_provider(provider: &'static Provider, api_key: String, model: String) -> LlmTarget {
        LlmTarget {
            kind: provider.kind,
            endpoint: provider.endpoint.to_string(),
            api_key,
            model,
            name: provider.name,
            debug_id: provider.id,
            extra_headers: provider.extra_headers,
        }
    }
}

/// One ask's resolved pair plus the reasoning-skip verdict — the product both
/// resolution paths (Custom in commands.rs, Default below) hand `run_ask`.
pub struct AskTargets {
    pub answer: LlmTarget,
    pub rewrite: LlmTarget,
    /// Whether the pre-search query rewrite is a known-doomed call for the
    /// rewrite model (its reply lands outside the parsed content) and should
    /// be skipped outright.
    pub rewrite_skip_reasoning: bool,
}

const VAR_API_KEY: &str = "WIKILENS_DEFAULT_API_KEY";
const VAR_API_PROVIDER: &str = "WIKILENS_DEFAULT_API_PROVIDER";
const VAR_API_URL: &str = "WIKILENS_DEFAULT_API_URL";
const VAR_ANSWER_MODEL: &str = "WIKILENS_DEFAULT_ANSWER_MODEL";
const VAR_REWRITE_MODEL: &str = "WIKILENS_DEFAULT_REWRITE_MODEL";

/// Pure Default-mode resolver over an injected env lookup (the
/// `resolve_model` purity precedent — the multi-threaded suite never mutates
/// env). `Err` is complete user-facing copy: it names the missing/invalid var
/// NAMES — never their values — and points at Settings.
///
/// Contract: the four `VAR_*` above are required; `WIKILENS_DEFAULT_API_PROVIDER`
/// selects the wire protocol (`anthropic` | `openai`, case-insensitive —
/// never a vendor id, so a neutral proxy is expressible);
/// `WIKILENS_DEFAULT_REWRITE_MODEL` is optional and falls back to the answer
/// model.
pub fn resolve_default_targets(
    env: impl Fn(&str) -> Option<String>,
) -> Result<AskTargets, String> {
    let api_key = env(VAR_API_KEY);
    let protocol = env(VAR_API_PROVIDER);
    let endpoint = env(VAR_API_URL);
    let answer_model = env(VAR_ANSWER_MODEL);
    let missing: Vec<&str> = [
        (VAR_API_KEY, api_key.is_some()),
        (VAR_API_PROVIDER, protocol.is_some()),
        (VAR_API_URL, endpoint.is_some()),
        (VAR_ANSWER_MODEL, answer_model.is_some()),
    ]
    .iter()
    .filter(|(_, present)| !present)
    .map(|(name, _)| *name)
    .collect();
    if !missing.is_empty() {
        return Err(format!(
            "Default mode isn't set up: {} not set. Set the WIKILENS_DEFAULT_* \
             variables — or pick a different answer source in Settings.",
            missing.join(", ")
        ));
    }
    let (Some(api_key), Some(protocol), Some(endpoint), Some(answer_model)) =
        (api_key, protocol, endpoint, answer_model)
    else {
        unreachable!("the missing-var check above covers all four");
    };
    let kind = match protocol.to_ascii_lowercase().as_str() {
        "anthropic" => ProviderKind::Anthropic,
        "openai" => ProviderKind::OpenAiCompatible,
        // Never echo the given value — the copy stays value-free by contract.
        _ => {
            return Err(format!(
                "Default mode is misconfigured: {VAR_API_PROVIDER} must be \
                 \"anthropic\" or \"openai\" — fix it, or pick a different \
                 answer source in Settings."
            ));
        }
    };
    let rewrite_model = env(VAR_REWRITE_MODEL).unwrap_or_else(|| answer_model.clone());
    // Off-registry model: the id suffix is the only conclusive reasoning
    // signal (no curated overlay here); the session breaker backstops the
    // unknown rest.
    let rewrite_skip_reasoning = crate::models::reasoning_from_id(&rewrite_model) == Some(true);
    let target = |model: String| LlmTarget {
        kind,
        endpoint: endpoint.clone(),
        api_key: api_key.clone(),
        model,
        name: DEFAULT_TARGET_NAME,
        debug_id: DEFAULT_TARGET_DEBUG_ID,
        extra_headers: &[],
    };
    Ok(AskTargets {
        answer: target(answer_model),
        rewrite: target(rewrite_model),
        rewrite_skip_reasoning,
    })
}

/// The impure half: real env reads (trimmed, blanks dropped). One function
/// feeds the settings panel's `configured`, `run_ask`'s Default arm, and the
/// first-launch auto-sense in `lib.rs` — the three can't disagree.
pub(crate) fn sense_default_targets() -> Result<AskTargets, String> {
    resolve_default_targets(crate::providers::env_nonempty)
}

/// The footer chip's word and the only provider label Local mode ever shows.
/// Renaming the tier = changing these two consts (+ the frontend's synthesized
/// "Local" provider group).
#[allow(dead_code)] // wired up by the backend cut; the attribute dies there
pub const LOCAL_TARGET_NAME: &str = "Local";
#[allow(dead_code)]
pub const LOCAL_TARGET_DEBUG_ID: &str = "local";

/// The base URL Local mode uses until the player stores their own — Ollama's
/// OpenAI-compatible root on its default port. Because this fallback always
/// exists, Local mode is never "not set up": a wrong server surfaces as a
/// connection error, not a setup error.
#[allow(dead_code)]
pub const LOCAL_DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";

/// Normalize a pasted Local AI base URL. `Ok(None)` means the field was
/// cleared — store nothing and fall back to [`LOCAL_DEFAULT_BASE_URL`].
/// Rules: trim; a scheme-less paste gets `http://` (local servers are the
/// common case — `localhost:11434` must just work); http(s) only; trailing
/// slashes drop; a bare origin gets `/v1` appended (every supported runtime
/// serves the OpenAI surface there); any other path is kept verbatim so
/// `/api/v1`-style proxies stay expressible. Query/fragment are dropped —
/// a base URL has neither. `Err` is complete user-facing copy; unlike the
/// key paths, echoing is a non-issue (a URL is config, not a secret), but
/// the copy leads with the fix anyway.
#[allow(dead_code)]
pub fn normalize_local_base_url(input: &str) -> Result<Option<String>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let invalid = || {
        String::from(
            "That doesn't look like a server address — use something like \
             http://localhost:11434.",
        )
    };
    let url = reqwest::Url::parse(&with_scheme).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid());
    }
    let host = url.host_str().ok_or_else(invalid)?;
    let mut base = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        base.push_str(&format!(":{port}"));
    }
    let path = url.path().trim_end_matches('/');
    if path.is_empty() {
        base.push_str("/v1");
    } else {
        base.push_str(path);
    }
    Ok(Some(base))
}

/// The base URL Local mode actually uses: the stored one, or the baked
/// Ollama default.
#[allow(dead_code)]
pub fn effective_local_base_url(stored: Option<&str>) -> String {
    stored
        .map(str::to_string)
        .unwrap_or_else(|| LOCAL_DEFAULT_BASE_URL.to_string())
}

/// The two endpoints derived from a Local base URL. Tolerates a trailing
/// slash so a hand-edited settings.json can't double it.
#[allow(dead_code)]
pub fn local_chat_endpoint(base: &str) -> String {
    format!("{}/chat/completions", base.trim_end_matches('/'))
}
#[allow(dead_code)]
pub fn local_models_endpoint(base: &str) -> String {
    format!("{}/models", base.trim_end_matches('/'))
}

/// Pure Local-mode resolver: the stored base URL (settings), the stored key
/// (empty = keyless server; llm.rs then omits the auth header entirely), and
/// the model the request picked. Always the OpenAI-compatible protocol —
/// every supported runtime (Ollama, LM Studio, llama.cpp, vLLM) speaks it, so
/// there is no protocol picker to misconfigure. Answer and rewrite are the
/// same target (identical `debug_id`s keep the debug table's rewrite-row
/// suppression working); the `:thinking` id suffix is the only conclusive
/// reasoning signal here, with the session breaker backstopping the rest.
/// `Err` is complete user-facing copy.
#[allow(dead_code)]
pub fn resolve_local_targets(
    stored_base_url: Option<&str>,
    api_key: String,
    model: &str,
) -> Result<AskTargets, String> {
    let model = model.trim();
    if model.is_empty() {
        return Err(String::from(
            "Pick a model for Local AI from the footer menu first.",
        ));
    }
    // The store only holds normalized values, but settings.json is a plain
    // file — re-validate so a hand-edited entry fails with a pointer, not a
    // confusing connection error against a mangled URL.
    let base = match stored_base_url {
        Some(stored) => normalize_local_base_url(stored)
            .map_err(|_| {
                String::from(
                    "The Local AI server address in Settings isn't a valid URL — \
                     fix it in Settings → Answers.",
                )
            })?
            .unwrap_or_else(|| LOCAL_DEFAULT_BASE_URL.to_string()),
        None => LOCAL_DEFAULT_BASE_URL.to_string(),
    };
    let endpoint = local_chat_endpoint(&base);
    let rewrite_skip_reasoning = crate::models::reasoning_from_id(model) == Some(true);
    let target = || LlmTarget {
        kind: ProviderKind::OpenAiCompatible,
        endpoint: endpoint.clone(),
        api_key: api_key.clone(),
        model: model.to_string(),
        name: LOCAL_TARGET_NAME,
        debug_id: LOCAL_TARGET_DEBUG_ID,
        extra_headers: &[],
    };
    Ok(AskTargets {
        answer: target(),
        rewrite: target(),
        rewrite_skip_reasoning,
    })
}

/// Env vars WikiLens once read and no longer does — the vendor key vars died
/// with the DPAPI store, the rewrite pins with "the picked model drives the
/// rewrite". One stderr line per stale var at startup (the returning-dev
/// safety net), naming the var and today's path; values never echoed. Note:
/// the `--ignored` live test suites still read the key vars deliberately —
/// a dev who keeps them exported just sees this nudge each launch.
pub fn legacy_env_notices(env: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let mut notices = Vec::new();
    for var in ["ANTHROPIC_API_KEY", "DEEPSEEK_API_KEY", "OPENROUTER_API_KEY"] {
        if env(var).is_some() {
            notices.push(format!(
                "wikilens: {var} is set but WikiLens no longer reads it — paste \
                 the key in Settings instead."
            ));
        }
    }
    for var in ["WIKILENS_REWRITE_MODEL", "WIKILENS_REWRITE_PROVIDER"] {
        if env(var).is_some() {
            notices.push(format!(
                "wikilens: {var} is set but WikiLens no longer reads it — the \
                 picked model drives the rewrite (Default mode: \
                 WIKILENS_DEFAULT_REWRITE_MODEL)."
            ));
        }
    }
    notices
}

/// Print the nudges (called once at startup, right after dotenv).
pub(crate) fn warn_legacy_env() {
    for notice in legacy_env_notices(crate::providers::env_nonempty) {
        eprintln!("{notice}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete, valid Default env; tests override single entries.
    fn full_env(name: &str) -> Option<String> {
        match name {
            "WIKILENS_DEFAULT_API_KEY" => Some("test-key".to_string()),
            "WIKILENS_DEFAULT_API_PROVIDER" => Some("anthropic".to_string()),
            "WIKILENS_DEFAULT_API_URL" => Some("https://proxy.example/v1/messages".to_string()),
            "WIKILENS_DEFAULT_ANSWER_MODEL" => Some("some-answer-model".to_string()),
            _ => None,
        }
    }

    #[test]
    fn resolver_requires_each_of_the_four_vars() {
        for missing in [
            "WIKILENS_DEFAULT_API_KEY",
            "WIKILENS_DEFAULT_API_PROVIDER",
            "WIKILENS_DEFAULT_API_URL",
            "WIKILENS_DEFAULT_ANSWER_MODEL",
        ] {
            let err = resolve_default_targets(|n| if n == missing { None } else { full_env(n) })
                .err()
                .unwrap_or_else(|| panic!("should fail without {missing}"));
            assert!(err.contains(missing), "must name {missing}: {err}");
            assert!(err.contains("Settings"), "must point at Settings: {err}");
            assert!(err.contains("answer source"), "must offer the way out: {err}");
        }
    }

    #[test]
    fn resolver_lists_every_missing_var_at_once() {
        let err = resolve_default_targets(|n| {
            if n == "WIKILENS_DEFAULT_API_KEY" || n == "WIKILENS_DEFAULT_ANSWER_MODEL" {
                None
            } else {
                full_env(n)
            }
        })
        .err()
        .expect("two missing vars");
        assert!(err.contains("WIKILENS_DEFAULT_API_KEY"), "{err}");
        assert!(err.contains("WIKILENS_DEFAULT_ANSWER_MODEL"), "{err}");
    }

    #[test]
    fn provider_parses_case_insensitively() {
        for (value, kind) in [
            ("Anthropic", ProviderKind::Anthropic),
            ("OPENAI", ProviderKind::OpenAiCompatible),
        ] {
            let targets = resolve_default_targets(|n| {
                if n == "WIKILENS_DEFAULT_API_PROVIDER" {
                    Some(value.to_string())
                } else {
                    full_env(n)
                }
            })
            .expect("valid protocol word");
            assert_eq!(targets.answer.kind, kind, "for {value}");
        }
    }

    /// Vendor ids are deliberately not protocol words — a neutral proxy must
    /// be expressible without impersonating a shipped vendor.
    #[test]
    fn vendor_or_garbage_provider_is_invalid() {
        for value in ["deepseek", "openrouter", "banana"] {
            let err = resolve_default_targets(|n| {
                if n == "WIKILENS_DEFAULT_API_PROVIDER" {
                    Some(value.to_string())
                } else {
                    full_env(n)
                }
            })
            .err()
            .unwrap_or_else(|| panic!("{value} should be invalid"));
            assert!(err.contains("WIKILENS_DEFAULT_API_PROVIDER"), "{err}");
            assert!(err.contains("\"anthropic\" or \"openai\""), "{err}");
            assert!(!err.contains(value), "must not echo the value: {err}");
        }
    }

    #[test]
    fn rewrite_falls_back_to_answer_model() {
        let targets = resolve_default_targets(full_env).expect("configured");
        assert_eq!(targets.answer.model, "some-answer-model");
        assert_eq!(targets.rewrite.model, "some-answer-model");
        assert_eq!(targets.answer.endpoint, targets.rewrite.endpoint);
        assert_eq!(targets.answer.api_key, targets.rewrite.api_key);
        assert_eq!(targets.answer.name, "Default");
        assert_eq!(targets.rewrite.debug_id, "default");
        assert!(!targets.rewrite_skip_reasoning, "plain id stays eligible");
        assert!(targets.answer.extra_headers.is_empty());
    }

    #[test]
    fn rewrite_override_applies_and_thinking_suffix_skips() {
        let targets = resolve_default_targets(|n| {
            if n == "WIKILENS_DEFAULT_REWRITE_MODEL" {
                Some("fast-model:thinking".to_string())
            } else {
                full_env(n)
            }
        })
        .expect("configured");
        assert_eq!(targets.rewrite.model, "fast-model:thinking");
        assert_eq!(targets.answer.model, "some-answer-model");
        assert!(targets.rewrite_skip_reasoning, ":thinking is conclusive");
    }

    /// The resolver's error copy must never echo env values — seed every var
    /// with a sentinel and drive each failure shape.
    #[test]
    fn error_copy_never_echoes_values() {
        const SENTINEL: &str = "WIKILENS-SENTINEL-NOT-A-REAL-VALUE";
        // Failure shape 1: invalid protocol word (all vars sentinel-valued).
        let err = resolve_default_targets(|_| Some(SENTINEL.to_string()))
            .err()
            .expect("sentinel protocol is invalid");
        assert!(!err.contains(SENTINEL), "echoed a value: {err}");
        // Failure shape 2: missing vars while the present ones hold sentinels.
        let err = resolve_default_targets(|n| {
            (n == "WIKILENS_DEFAULT_API_KEY").then(|| SENTINEL.to_string())
        })
        .err()
        .expect("three missing vars");
        assert!(!err.contains(SENTINEL), "echoed a value: {err}");
    }

    #[test]
    fn legacy_notices_name_each_stale_var_and_never_echo_values() {
        const SENTINEL: &str = "WIKILENS-SENTINEL-NOT-A-REAL-VALUE";
        // Clean env → silence.
        assert!(legacy_env_notices(|_| None).is_empty());

        // Each dead var earns exactly one notice naming it; values stay out.
        for var in [
            "ANTHROPIC_API_KEY",
            "DEEPSEEK_API_KEY",
            "OPENROUTER_API_KEY",
            "WIKILENS_REWRITE_MODEL",
            "WIKILENS_REWRITE_PROVIDER",
        ] {
            let notices =
                legacy_env_notices(|n| (n == var).then(|| SENTINEL.to_string()));
            assert_eq!(notices.len(), 1, "{var}");
            assert!(notices[0].contains(var), "{var}: {}", notices[0]);
            assert!(!notices[0].contains(SENTINEL), "echoed a value: {}", notices[0]);
        }

        // Live vars never trigger it.
        for var in ["WIKILENS_DEFAULT_API_KEY", "WIKILENS_QUERY_REWRITE"] {
            assert!(
                legacy_env_notices(|n| (n == var).then(|| "1".to_string())).is_empty(),
                "{var} is not legacy"
            );
        }
    }

    #[test]
    fn base_url_normalization_matrix() {
        // (input, expected stored value)
        for (input, expected) in [
            // Scheme-less pastes get http:// — the local common case.
            ("localhost:11434", "http://localhost:11434/v1"),
            ("192.168.0.5:8080", "http://192.168.0.5:8080/v1"),
            // Bare origins gain /v1; trailing slashes drop first.
            ("http://localhost:11434", "http://localhost:11434/v1"),
            ("http://localhost:11434/", "http://localhost:11434/v1"),
            // An explicit path is kept verbatim (minus trailing slashes).
            ("http://localhost:11434/v1", "http://localhost:11434/v1"),
            ("http://localhost:11434/v1/", "http://localhost:11434/v1"),
            ("https://box.lan/api/v1", "https://box.lan/api/v1"),
            // Whitespace trims; https survives.
            ("  https://box.lan:9090  ", "https://box.lan:9090/v1"),
        ] {
            assert_eq!(
                normalize_local_base_url(input).expect(input).as_deref(),
                Some(expected),
                "for {input:?}"
            );
        }
    }

    #[test]
    fn empty_base_url_input_clears_to_the_default() {
        for input in ["", "   "] {
            assert_eq!(normalize_local_base_url(input), Ok(None), "for {input:?}");
        }
        assert_eq!(effective_local_base_url(None), LOCAL_DEFAULT_BASE_URL);
        assert_eq!(
            effective_local_base_url(Some("http://box.lan/v1")),
            "http://box.lan/v1"
        );
    }

    #[test]
    fn garbage_base_url_is_a_friendly_error() {
        for input in ["ftp://box.lan", "http://", "ht tp://x", "///"] {
            let err = normalize_local_base_url(input)
                .err()
                .unwrap_or_else(|| panic!("{input:?} should be invalid"));
            assert!(err.contains("http://localhost:11434"), "{err}");
        }
    }

    #[test]
    fn local_endpoints_derive_from_the_base() {
        assert_eq!(
            local_chat_endpoint("http://localhost:11434/v1"),
            "http://localhost:11434/v1/chat/completions"
        );
        // Trailing-slash tolerance: a hand-edited store can't double it.
        assert_eq!(
            local_models_endpoint("http://box.lan/api/v1/"),
            "http://box.lan/api/v1/models"
        );
    }

    #[test]
    fn local_resolver_builds_the_openai_pair() {
        let targets = resolve_local_targets(None, String::new(), "llama3.2:3b")
            .expect("default base resolves");
        assert_eq!(targets.answer.kind, ProviderKind::OpenAiCompatible);
        assert_eq!(
            targets.answer.endpoint,
            "http://localhost:11434/v1/chat/completions"
        );
        assert!(targets.answer.api_key.is_empty(), "keyless stays keyless");
        assert_eq!(targets.answer.model, "llama3.2:3b");
        assert_eq!(targets.answer.name, "Local");
        assert_eq!(targets.answer.debug_id, "local");
        assert!(targets.answer.extra_headers.is_empty());
        // Answer == rewrite: same model, same endpoint, same debug id — the
        // debug table's rewrite-row suppression relies on the pair matching.
        assert_eq!(targets.rewrite.model, targets.answer.model);
        assert_eq!(targets.rewrite.endpoint, targets.answer.endpoint);
        assert_eq!(targets.rewrite.debug_id, targets.answer.debug_id);
        assert!(!targets.rewrite_skip_reasoning, "plain id stays eligible");
    }

    #[test]
    fn local_resolver_uses_the_stored_base_and_key() {
        let targets = resolve_local_targets(
            Some("http://box.lan:8080/api/v1"),
            "tok-1".to_string(),
            "qwen3:8b",
        )
        .expect("stored base resolves");
        assert_eq!(
            targets.answer.endpoint,
            "http://box.lan:8080/api/v1/chat/completions"
        );
        assert_eq!(targets.answer.api_key, "tok-1");
    }

    #[test]
    fn local_resolver_requires_a_model_pick() {
        for model in ["", "   "] {
            let err = resolve_local_targets(None, String::new(), model)
                .err()
                .expect("blank model must not resolve");
            assert!(err.contains("footer menu"), "{err}");
        }
    }

    #[test]
    fn local_resolver_flags_a_thinking_rewrite() {
        let targets = resolve_local_targets(None, String::new(), "some-model:thinking")
            .expect("resolves");
        assert!(targets.rewrite_skip_reasoning, ":thinking is conclusive");
    }

    /// A hand-edited settings.json can hold anything; the resolver must fail
    /// with a Settings pointer, not hand llm.rs a mangled URL.
    #[test]
    fn local_resolver_rejects_a_corrupt_stored_base() {
        let err = resolve_local_targets(Some("ftp://box.lan"), String::new(), "m")
            .err()
            .expect("corrupt stored base must not resolve");
        assert!(err.contains("Settings"), "{err}");
    }

    #[test]
    fn from_provider_maps_registry_fields() {
        let anthropic = crate::providers::find_provider("anthropic").expect("registry");
        let target =
            LlmTarget::from_provider(anthropic, "sk-1".to_string(), "some-model".to_string());
        assert_eq!(target.kind, ProviderKind::Anthropic);
        assert_eq!(target.endpoint, anthropic.endpoint);
        assert_eq!(target.name, "Anthropic");
        assert_eq!(target.debug_id, "anthropic");
        assert!(target.extra_headers.is_empty());

        let openrouter = crate::providers::find_provider("openrouter").expect("registry");
        let target =
            LlmTarget::from_provider(openrouter, "sk-2".to_string(), "some-model".to_string());
        assert_eq!(target.kind, ProviderKind::OpenAiCompatible);
        // Slice identity: the registry's attribution headers ride along.
        assert!(std::ptr::eq(target.extra_headers, openrouter.extra_headers));
    }
}
