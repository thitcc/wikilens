//! Resolved LLM call targets — the seam between "where the model and
//! credential come from" (registry + `KeyStore` in Custom mode; the stored
//! base URL + optional key in Local mode) and the wire client (`llm.rs`).
//! Nothing downstream of an [`LlmTarget`] knows or cares which mode produced
//! it (vault/2026-08-24_replace-default-mode-with-local-ai.md).

use crate::providers::{Provider, ProviderKind};

/// Everything one LLM call needs. Runtime strings for endpoint/key/model;
/// `name`/`debug_id` stay `&'static str` because `AppError::Llm.provider` is
/// pinned `&'static str` (config_guardrails — a runtime key value structurally
/// can't be embedded) and both sources are static: registry entries, or the
/// `LOCAL_TARGET_*` consts below.
pub struct LlmTarget {
    pub kind: ProviderKind,
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    /// User-facing provider label for error copy ("Local" in Local mode).
    pub name: &'static str,
    /// Debug-table/window id — feeds the "provider / model" pair strings
    /// whose equality suppresses the rewrite row (debug.rs). Identical
    /// targets must carry identical ids.
    pub debug_id: &'static str,
    /// Kept `'static` (registry slices or `&[]`). The Anthropic request
    /// builders deliberately ignore these — llm.rs preserves that asymmetry.
    pub extra_headers: &'static [(&'static str, &'static str)],
}

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
/// resolution paths (Custom in commands.rs, Local below) hand `run_ask`.
pub struct AskTargets {
    pub answer: LlmTarget,
    pub rewrite: LlmTarget,
    /// Whether the pre-search query rewrite is a known-doomed call for the
    /// rewrite model (its reply lands outside the parsed content) and should
    /// be skipped outright.
    pub rewrite_skip_reasoning: bool,
}

/// The footer chip's word and the only provider label Local mode ever shows.
/// Renaming the tier = changing these two consts (+ the frontend's synthesized
/// "Local" provider group).
pub const LOCAL_TARGET_NAME: &str = "Local";
pub const LOCAL_TARGET_DEBUG_ID: &str = "local";

/// The base URL Local mode uses until the player stores their own — Ollama's
/// OpenAI-compatible root on its default port. Because this fallback always
/// exists, Local mode is never "not set up": a wrong server surfaces as a
/// connection error, not a setup error.
pub const LOCAL_DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";

/// The scheme a scheme-less paste defaults to. `http` only for hosts that
/// plainly live on this machine or LAN — loopback, private/link-local IPs,
/// dotless names, the household pseudo-TLDs; anything routable defaults to
/// `https`, because a stored key must never silently ride plaintext to a
/// public host (typing `http://` explicitly stays allowed — LAN reverse
/// proxies are a supported case, same stance as the wiki fetch policy).
fn scheme_less_default(host: &str) -> &'static str {
    use std::net::IpAddr;
    let name = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = name.parse::<IpAddr>() {
        let local = match ip {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            IpAddr::V6(v6) => {
                v6.is_loopback()
                    // fc00::/7 (unique-local) and fe80::/10 (link-local).
                    || (v6.segments()[0] & 0xfe00) == 0xfc00
                    || (v6.segments()[0] & 0xffc0) == 0xfe80
            }
        };
        return if local { "http" } else { "https" };
    }
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    let household = [".local", ".lan", ".home", ".internal", ".localdomain"];
    if !name.contains('.') || household.iter().any(|tld| name.ends_with(tld)) {
        "http"
    } else {
        "https"
    }
}

/// Normalize a pasted Local AI base URL. `Ok(None)` means the field was
/// cleared — store nothing and fall back to [`LOCAL_DEFAULT_BASE_URL`].
/// Rules: trim; the `http:/host` single-slash typo is repaired (Url::parse
/// would read the scheme word as the HOST); a scheme-less paste gets
/// `http://` for a local-looking host (`localhost:11434` must just work) and
/// `https://` for a routable one (see [`scheme_less_default`]); http(s)
/// only; trailing slashes drop; a pasted full endpoint (`…/chat/completions`
/// or `…/models` — the URLs LM Studio's UI hands out) is trimmed back to its
/// base; a bare origin gains `/v1` (every supported runtime serves the
/// OpenAI surface there); any other path is kept verbatim so `/api/v1`-style
/// proxies stay expressible. Query/fragment are dropped — a base URL has
/// neither. `Err` is complete user-facing copy; unlike the key paths,
/// echoing is a non-issue (a URL is config, not a secret), but the copy
/// leads with the fix anyway.
pub fn normalize_local_base_url(input: &str) -> Result<Option<String>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let invalid = || {
        String::from(
            "That doesn't look like a server address — use something like \
             http://localhost:11434.",
        )
    };
    let repaired = repair_single_slash_scheme(trimmed);
    let with_scheme = if repaired.contains("://") {
        repaired.to_string()
    } else {
        // Probe-parse once to learn the host, then pick the scheme by it.
        let probe = reqwest::Url::parse(&format!("http://{repaired}")).map_err(|_| invalid())?;
        let host = probe.host_str().ok_or_else(invalid)?;
        format!("{}://{repaired}", scheme_less_default(host))
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
    let path = path.strip_suffix("/chat/completions").unwrap_or(path);
    let path = path.strip_suffix("/models").unwrap_or(path);
    if path.is_empty() {
        base.push_str("/v1");
    } else {
        base.push_str(path);
    }
    Ok(Some(base))
}

/// `"http:/host"` → `"http://host"` (and the https twin). Only the exact
/// one-slash form is touched; everything else passes through for the parser
/// to judge.
fn repair_single_slash_scheme(input: &str) -> String {
    for scheme in ["http", "https"] {
        let prefix = format!("{scheme}:/");
        if let Some(rest) = input.strip_prefix(&prefix) {
            if !rest.starts_with('/') {
                return format!("{scheme}://{rest}");
            }
        }
    }
    input.to_string()
}

/// The base URL Local mode actually uses: the stored one, or the baked
/// Ollama default. Display-oriented (the Settings field shows it verbatim,
/// mangled or not, so a hand-edited entry is visible and fixable) — the
/// request paths go through [`resolved_local_base`], which validates.
pub fn effective_local_base_url(stored: Option<&str>) -> String {
    stored
        .map(str::to_string)
        .unwrap_or_else(|| LOCAL_DEFAULT_BASE_URL.to_string())
}

/// The validated base every request path shares — the ask resolver and the
/// model-list route must never disagree about a hand-edited settings.json.
/// Re-normalizes the stored value (the store keeps it verbatim at load);
/// `Err` is complete user-facing copy pointing at Settings.
pub fn resolved_local_base(stored: Option<&str>) -> Result<String, String> {
    match stored {
        Some(stored) => Ok(normalize_local_base_url(stored)
            .map_err(|_| {
                String::from(
                    "The Local AI server address in Settings isn't a valid URL — \
                     fix it in Settings → Answers.",
                )
            })?
            .unwrap_or_else(|| LOCAL_DEFAULT_BASE_URL.to_string())),
        None => Ok(LOCAL_DEFAULT_BASE_URL.to_string()),
    }
}

/// The two endpoints derived from a Local base URL. Tolerates a trailing
/// slash so a hand-edited settings.json can't double it.
pub fn local_chat_endpoint(base: &str) -> String {
    format!("{}/chat/completions", base.trim_end_matches('/'))
}
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
    // file — the shared validation makes a hand-edited entry fail with a
    // pointer, not a confusing connection error against a mangled URL.
    let base = resolved_local_base(stored_base_url)?;
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
/// rewrite", the `WIKILENS_DEFAULT_*` family with Default mode itself. One
/// stderr line per stale var at startup (the returning-dev safety net),
/// naming the var and today's path; values never echoed. Note: the
/// `--ignored` live test suites still read the key vars deliberately — a dev
/// who keeps them exported just sees this nudge each launch.
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
                 picked model drives the rewrite."
            ));
        }
    }
    for var in [
        "WIKILENS_DEFAULT_API_KEY",
        "WIKILENS_DEFAULT_API_PROVIDER",
        "WIKILENS_DEFAULT_API_URL",
        "WIKILENS_DEFAULT_ANSWER_MODEL",
        "WIKILENS_DEFAULT_REWRITE_MODEL",
        "WIKILENS_DEFAULT_VISION",
    ] {
        if env(var).is_some() {
            notices.push(format!(
                "wikilens: {var} is set but WikiLens no longer reads it — \
                 Default mode was replaced by Local AI (Settings → Answers)."
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
            // The whole Default-mode family died with the mode itself.
            "WIKILENS_DEFAULT_API_KEY",
            "WIKILENS_DEFAULT_API_PROVIDER",
            "WIKILENS_DEFAULT_API_URL",
            "WIKILENS_DEFAULT_ANSWER_MODEL",
            "WIKILENS_DEFAULT_REWRITE_MODEL",
            "WIKILENS_DEFAULT_VISION",
        ] {
            let notices =
                legacy_env_notices(|n| (n == var).then(|| SENTINEL.to_string()));
            assert_eq!(notices.len(), 1, "{var}");
            assert!(notices[0].contains(var), "{var}: {}", notices[0]);
            assert!(!notices[0].contains(SENTINEL), "echoed a value: {}", notices[0]);
        }

        // Live vars never trigger it.
        for var in ["WIKILENS_QUERY_REWRITE", "WIKILENS_TITLE_INDEX"] {
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
            // Scheme-less local-looking pastes get http:// — the common case.
            ("localhost:11434", "http://localhost:11434/v1"),
            ("192.168.0.5:8080", "http://192.168.0.5:8080/v1"),
            ("box", "http://box/v1"),
            ("box.lan:8080", "http://box.lan:8080/v1"),
            // A scheme-less ROUTABLE host defaults to https — a stored key
            // must never silently ride plaintext to a public host.
            ("myproxy.example.com", "https://myproxy.example.com/v1"),
            ("203.0.113.7:8443", "https://203.0.113.7:8443/v1"),
            // Explicit http to a routable host stays allowed (LAN proxies).
            ("http://myproxy.example.com", "http://myproxy.example.com/v1"),
            // The single-slash scheme typo is repaired, not stored as
            // host "http".
            ("http:/localhost:11434", "http://localhost:11434/v1"),
            // Bare origins gain /v1; trailing slashes drop first.
            ("http://localhost:11434", "http://localhost:11434/v1"),
            ("http://localhost:11434/", "http://localhost:11434/v1"),
            // An explicit path is kept verbatim (minus trailing slashes).
            ("http://localhost:11434/v1", "http://localhost:11434/v1"),
            ("http://localhost:11434/v1/", "http://localhost:11434/v1"),
            ("https://box.lan/api/v1", "https://box.lan/api/v1"),
            // A pasted full endpoint (the URL LM Studio's UI hands out)
            // trims back to its base instead of doubling the path later.
            (
                "http://localhost:1234/v1/chat/completions",
                "http://localhost:1234/v1",
            ),
            ("http://localhost:1234/v1/models", "http://localhost:1234/v1"),
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

    /// The one validated base both request paths share — the ask resolver and
    /// the model-list route must agree about a hand-edited store.
    #[test]
    fn resolved_local_base_validates_like_the_resolver() {
        assert_eq!(
            resolved_local_base(None).as_deref(),
            Ok(LOCAL_DEFAULT_BASE_URL)
        );
        // A verbatim scheme-less stored value re-normalizes here too.
        assert_eq!(
            resolved_local_base(Some("localhost:11434")).as_deref(),
            Ok("http://localhost:11434/v1")
        );
        let err = resolved_local_base(Some("ftp://box.lan"))
            .expect_err("corrupt stored base must not resolve");
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
