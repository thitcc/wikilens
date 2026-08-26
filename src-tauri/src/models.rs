//! Per-provider model catalogs: live fetch + parsing.
//!
//! The hybrid sourcing strategy (live fetch → session cache → curated
//! fallback) is decided in `vault/2026-07-05_model-list-sourcing.md`; the
//! cache and the fallback branching live in `commands::list_models`. This
//! module only knows how to fetch and parse. Parsers trim each payload to
//! `{id, label}` Rust-side, so OpenRouter's ~1–2 MB catalog crosses IPC at
//! roughly 30 KB.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::http;
use crate::providers::{CuratedModel, Provider, ProviderKind};

/// One selectable model, as sent to the frontend menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    /// Whether the model accepts image input (drives the "Image" badge). See
    /// the asymmetric parser defaults below and
    /// `vault/2026-07-06_model-vision-badges.md`.
    pub vision: bool,
    /// Three-state reasoning classification: `Some(true)` = thinks before
    /// answering (drives the "Reasoning" badge; `ask` skips the pre-search
    /// rewrite for such models), `Some(false)` = answers directly ("Fast"
    /// badge), `None` = unknown (no badge, no skip — omitted from the IPC
    /// payload). See `vault/2026-07-10_reasoning-skip-and-capability-tags.md`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
}

/// Where a model list came from. `Fallback` lets the UI show a muted
/// "offline list" note without learning *why* the live fetch was skipped
/// (key presence stays Rust-side).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelSource {
    Live,
    Fallback,
}

/// Shorter than the wiki's 12s: a slow catalog fetch only delays a menu whose
/// curated fallback is a fine experience, not a failed answer.
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// Reasoning classification from the id alone. Only OpenRouter's `:thinking`
/// suffix is conclusive — it selects the reasoning variant by contract.
/// Deliberately no fuzzy o1/o3/r1 matching: a wrong `Some(true)` would
/// silently disable a working rewrite for that model.
pub fn reasoning_from_id(id: &str) -> Option<bool> {
    id.ends_with(":thinking").then_some(true)
}

/// Fetch a provider's live model list. `api_key` is `None` for endpoints that
/// don't need auth (OpenRouter) — never send a key where it isn't needed.
pub async fn fetch_models(
    client: &reqwest::Client,
    provider: &Provider,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfo>, AppError> {
    fetch_models_at(client, provider.kind, provider.models_endpoint, provider.name, api_key).await
}

/// The fetch core, over runtime values instead of a registry entry — the Local
/// AI catalog has a user-configured endpoint that no `&'static Provider` can
/// carry. `provider_name` stays `&'static str` because it feeds
/// `AppError::Llm.provider`.
pub async fn fetch_models_at(
    client: &reqwest::Client,
    kind: ProviderKind,
    models_endpoint: &str,
    provider_name: &'static str,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfo>, AppError> {
    let mut request = client.get(models_endpoint).timeout(FETCH_TIMEOUT);
    if let Some(key) = api_key {
        request = match kind {
            ProviderKind::Anthropic => request
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01"),
            ProviderKind::OpenAiCompatible => {
                request.header("Authorization", format!("Bearer {key}"))
            }
        };
    }

    let resp = request.send().await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::Llm {
            provider: provider_name,
            status: status.as_u16(),
            body: http::read_error_body(resp).await,
        });
    }
    let body = http::read_body_capped(resp, http::MAX_RESPONSE_BYTES).await?;

    match kind {
        ProviderKind::Anthropic => parse_anthropic_models(&body),
        ProviderKind::OpenAiCompatible => parse_openai_models(&body),
    }
}

/// Parse the Anthropic `GET /v1/models` payload: `data[].id` +
/// `data[].display_name`. The registry endpoint asks for `limit=1000` and the
/// catalog is single-digit-sized, so no pagination loop.
fn parse_anthropic_models(body: &str) -> Result<Vec<ModelInfo>, AppError> {
    #[derive(Deserialize)]
    struct Entry {
        id: String,
        display_name: Option<String>,
        capabilities: Option<Capabilities>,
    }
    #[derive(Deserialize)]
    struct Capabilities {
        image_input: Option<ImageInput>,
    }
    #[derive(Deserialize)]
    struct ImageInput {
        supported: Option<bool>,
    }
    #[derive(Deserialize)]
    struct Payload {
        data: Vec<Entry>,
    }

    let payload: Payload = serde_json::from_str(body)
        .map_err(|e| AppError::Parse(format!("Anthropic model list: {e}")))?;
    Ok(payload
        .data
        .into_iter()
        .map(|entry| {
            let label = entry
                .display_name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| entry.id.clone());
            // Default true: every active Claude model is vision-capable, and a
            // schema hiccup (missing `capabilities`) must not un-badge the whole
            // catalog. An explicit `supported: false` is still honored.
            let vision = entry
                .capabilities
                .and_then(|c| c.image_input)
                .and_then(|i| i.supported)
                .unwrap_or(true);
            ModelInfo {
                id: entry.id,
                label,
                vision,
                // Provider-level fact, not per-model: WikiLens never sends a
                // thinking param, so every Claude model answers directly here.
                reasoning: Some(false),
            }
        })
        .collect())
}

/// Parse an OpenAI-style `GET /models` payload: `data[].id`, plus the
/// optional `data[].name` OpenRouter adds (DeepSeek sends bare ids).
fn parse_openai_models(body: &str) -> Result<Vec<ModelInfo>, AppError> {
    #[derive(Deserialize)]
    struct Entry {
        id: String,
        name: Option<String>,
        architecture: Option<Architecture>,
        supported_parameters: Option<Vec<String>>,
    }
    #[derive(Deserialize)]
    struct Architecture {
        input_modalities: Option<Vec<String>>,
    }
    #[derive(Deserialize)]
    struct Payload {
        data: Vec<Entry>,
    }

    let payload: Payload = serde_json::from_str(body)
        .map_err(|e| AppError::Parse(format!("model list: {e}")))?;
    Ok(payload
        .data
        .into_iter()
        .map(|entry| {
            let label = entry
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| entry.id.clone());
            // Default false: DeepSeek's bare entries have no `architecture` and
            // are text-only, so they naturally fall here. OpenRouter lists
            // `input_modalities`; a model is vision-capable iff it includes
            // "image". Wrongly badging is worse than a missing badge.
            let vision = entry
                .architecture
                .and_then(|a| a.input_modalities)
                .map(|mods| mods.iter().any(|m| m == "image"))
                .unwrap_or(false);
            // Conservative three-state: a `:thinking` id is Reasoning by
            // contract; `supported_parameters` (OpenRouter only) proves
            // fastness by *absence* of "reasoning", never reasoning-ness by
            // presence (hybrid models list it but answer directly by
            // default). DeepSeek's bare entries have neither → unknown, and
            // the curated overlay fills them in (`overlay_curated_reasoning`).
            let reasoning =
                reasoning_from_id(&entry.id).or_else(|| match &entry.supported_parameters {
                    Some(params) if !params.iter().any(|p| p == "reasoning") => Some(false),
                    _ => None,
                });
            ModelInfo {
                id: entry.id,
                label,
                vision,
                reasoning,
            }
        })
        .collect())
}

/// Fill `reasoning: None` gaps in a live list from the curated registry.
/// Only gaps: a live `Some` is the provider's own declaration and wins.
/// This is what badges DeepSeek's live list — its `/models` payload carries
/// no capability fields, but both v4 models are curated.
fn overlay_curated_reasoning(models: &mut [ModelInfo], curated: &[CuratedModel]) {
    for model in models.iter_mut().filter(|m| m.reasoning.is_none()) {
        model.reasoning = curated
            .iter()
            .find(|c| c.id == model.id)
            .map(|c| c.reasoning);
    }
}

/// Pick the list to serve: a live one when the fetch produced anything,
/// otherwise the provider's curated fallback. An *empty* live list also
/// degrades — an empty menu group would be strictly worse than a stale one.
pub fn resolve_model_list(
    live: Result<Vec<ModelInfo>, AppError>,
    curated: &[CuratedModel],
) -> (Vec<ModelInfo>, ModelSource) {
    match live {
        Ok(mut models) if !models.is_empty() => {
            overlay_curated_reasoning(&mut models, curated);
            (models, ModelSource::Live)
        }
        _ => (
            curated
                .iter()
                .map(|m| ModelInfo {
                    id: m.id.to_string(),
                    label: m.label.to_string(),
                    vision: m.vision,
                    reasoning: Some(m.reasoning),
                })
                .collect(),
            ModelSource::Fallback,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Anthropic parser ----

    #[test]
    fn anthropic_parses_id_and_display_name() {
        let body = r#"{"data":[
            {"type":"model","id":"claude-sonnet-5","display_name":"Claude Sonnet 5","created_at":"2026-01-01T00:00:00Z"},
            {"type":"model","id":"claude-haiku-4-5-20251001","display_name":"Claude Haiku 4.5","created_at":"2025-10-01T00:00:00Z"}
        ],"has_more":false,"first_id":"a","last_id":"b"}"#;
        let models = parse_anthropic_models(body).unwrap();
        assert_eq!(
            models,
            vec![
                ModelInfo {
                    id: "claude-sonnet-5".into(),
                    label: "Claude Sonnet 5".into(),
                    vision: true,
                    reasoning: Some(false),
                },
                ModelInfo {
                    id: "claude-haiku-4-5-20251001".into(),
                    label: "Claude Haiku 4.5".into(),
                    vision: true,
                    reasoning: Some(false),
                },
            ]
        );
    }

    #[test]
    fn anthropic_vision_defaults_true_and_respects_explicit_false() {
        let body = r#"{"data":[
            {"id":"a","capabilities":{"image_input":{"supported":true}}},
            {"id":"b","capabilities":{"image_input":{"supported":false}}},
            {"id":"c"}
        ]}"#;
        let models = parse_anthropic_models(body).unwrap();
        assert!(models[0].vision, "explicit true");
        assert!(!models[1].vision, "explicit false is respected");
        assert!(models[2].vision, "absent capabilities → default true");
    }

    #[test]
    fn anthropic_label_falls_back_to_id_when_display_name_missing() {
        let body = r#"{"data":[{"id":"claude-sonnet-5"},{"id":"claude-x","display_name":"  "}]}"#;
        let models = parse_anthropic_models(body).unwrap();
        assert_eq!(models[0].label, "claude-sonnet-5");
        assert_eq!(models[1].label, "claude-x");
    }

    #[test]
    fn anthropic_rejects_malformed_and_shapeless_json() {
        assert!(parse_anthropic_models("not json at all").is_err());
        assert!(parse_anthropic_models(r#"{"models":[]}"#).is_err());
    }

    /// Provider-level fact: WikiLens never sends a thinking param, so every
    /// Anthropic model is Fast regardless of payload shape.
    #[test]
    fn anthropic_models_are_always_fast() {
        let body = r#"{"data":[{"id":"claude-sonnet-5"},{"id":"claude-opus-4-8"}]}"#;
        let models = parse_anthropic_models(body).unwrap();
        assert!(models.iter().all(|m| m.reasoning == Some(false)));
    }

    // ---- OpenAI-style parser (DeepSeek / OpenRouter) ----

    #[test]
    fn openai_parses_deepseek_bare_ids() {
        let body = r#"{"object":"list","data":[
            {"id":"deepseek-v4-flash","object":"model","owned_by":"deepseek"},
            {"id":"deepseek-v4-pro","object":"model","owned_by":"deepseek"}
        ]}"#;
        let models = parse_openai_models(body).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "deepseek-v4-flash");
        assert_eq!(models[0].label, "deepseek-v4-flash");
        // Bare entries have no `architecture` → text-only.
        assert!(!models[0].vision);
    }

    #[test]
    fn openai_vision_from_input_modalities() {
        let body = r#"{"data":[
            {"id":"vis","architecture":{"input_modalities":["text","image"]}},
            {"id":"txt","architecture":{"input_modalities":["text"]}},
            {"id":"bare"}
        ]}"#;
        let models = parse_openai_models(body).unwrap();
        assert!(models[0].vision, "input_modalities includes image");
        assert!(!models[1].vision, "text-only modalities");
        assert!(!models[2].vision, "no architecture → default false");
    }

    /// The conservative three-state read of OpenRouter's
    /// `supported_parameters`: absence of "reasoning" proves fastness, its
    /// presence proves nothing (hybrids answer directly by default), and a
    /// missing field (DeepSeek's bare entries) stays unknown.
    #[test]
    fn openai_reasoning_three_state_from_supported_parameters() {
        let body = r#"{"data":[
            {"id":"fast","supported_parameters":["temperature","top_p"]},
            {"id":"hybrid","supported_parameters":["temperature","reasoning"]},
            {"id":"bare"}
        ]}"#;
        let models = parse_openai_models(body).unwrap();
        assert_eq!(models[0].reasoning, Some(false), "no reasoning param → Fast");
        assert_eq!(models[1].reasoning, None, "hybrid → unknown, never Some(true)");
        assert_eq!(models[2].reasoning, None, "field absent → unknown");
    }

    #[test]
    fn openai_thinking_id_wins_over_supported_parameters() {
        let body = r#"{"data":[
            {"id":"anthropic/claude-sonnet-5:thinking","supported_parameters":["reasoning"]}
        ]}"#;
        let models = parse_openai_models(body).unwrap();
        assert_eq!(models[0].reasoning, Some(true));
    }

    #[test]
    fn reasoning_from_id_only_trusts_the_thinking_suffix() {
        assert_eq!(reasoning_from_id("anthropic/claude-sonnet-5:thinking"), Some(true));
        // Deliberately no fuzzy matching — these stay unknown.
        assert_eq!(reasoning_from_id("deepseek-v4-pro"), None);
        assert_eq!(reasoning_from_id("openai/o1-preview"), None);
        assert_eq!(reasoning_from_id("thinking"), None);
    }

    #[test]
    fn openai_prefers_openrouter_name_as_label() {
        let body = r#"{"data":[
            {"id":"openai/gpt-4o-mini","name":"OpenAI: GPT-4o-mini","context_length":128000,"pricing":{"prompt":"0.00000015"}}
        ]}"#;
        let models = parse_openai_models(body).unwrap();
        assert_eq!(
            models,
            vec![ModelInfo {
                id: "openai/gpt-4o-mini".into(),
                label: "OpenAI: GPT-4o-mini".into(),
                vision: false,   // no `architecture` in this fixture
                reasoning: None, // no `supported_parameters` either
            }]
        );
    }

    #[test]
    fn openai_rejects_malformed_and_shapeless_json() {
        assert!(parse_openai_models("<html>502</html>").is_err());
        assert!(parse_openai_models(r#"{"error":{"message":"nope"}}"#).is_err());
    }

    // ---- Resolution ----

    fn curated() -> Vec<CuratedModel> {
        vec![CuratedModel {
            id: "fallback-model",
            label: "Fallback Model",
            vision: true,
            reasoning: false,
        }]
    }

    #[test]
    fn resolve_prefers_non_empty_live_list() {
        let live = Ok(vec![ModelInfo {
            id: "live-model".into(),
            label: "Live Model".into(),
            vision: false,
            reasoning: None,
        }]);
        let (models, source) = resolve_model_list(live, &curated());
        assert_eq!(source, ModelSource::Live);
        assert_eq!(models[0].id, "live-model");
    }

    #[test]
    fn resolve_degrades_to_curated_on_error() {
        let live = Err(AppError::Parse("boom".into()));
        let (models, source) = resolve_model_list(live, &curated());
        assert_eq!(source, ModelSource::Fallback);
        assert_eq!(
            models,
            vec![ModelInfo {
                id: "fallback-model".into(),
                label: "Fallback Model".into(),
                vision: true,
                reasoning: Some(false),
            }]
        );
    }

    #[test]
    fn resolve_degrades_to_curated_on_empty_live_list() {
        let (models, source) = resolve_model_list(Ok(vec![]), &curated());
        assert_eq!(source, ModelSource::Fallback);
        assert_eq!(models.len(), 1);
    }

    #[test]
    fn fallback_list_carries_curated_vision() {
        let curated = [
            CuratedModel { id: "v", label: "V", vision: true, reasoning: false },
            CuratedModel { id: "t", label: "T", vision: false, reasoning: false },
        ];
        let (models, source) =
            resolve_model_list(Err(AppError::Parse("x".into())), &curated);
        assert_eq!(source, ModelSource::Fallback);
        assert!(models[0].vision);
        assert!(!models[1].vision);
    }

    #[test]
    fn fallback_list_carries_curated_reasoning() {
        let curated = [
            CuratedModel { id: "r", label: "R", vision: false, reasoning: true },
            CuratedModel { id: "f", label: "F", vision: false, reasoning: false },
        ];
        let (models, source) =
            resolve_model_list(Err(AppError::Parse("x".into())), &curated);
        assert_eq!(source, ModelSource::Fallback);
        // Curated entries are always classified — the fallback never shows an
        // unbadged row.
        assert_eq!(models[0].reasoning, Some(true));
        assert_eq!(models[1].reasoning, Some(false));
    }

    /// The overlay fills only `None` gaps: a live `Some` (the provider's own
    /// declaration) is never clobbered, and ids outside the curated list stay
    /// unknown.
    #[test]
    fn live_list_overlay_fills_only_none_gaps() {
        let curated = [
            CuratedModel { id: "gap", label: "Gap", vision: false, reasoning: true },
            CuratedModel { id: "declared", label: "Declared", vision: false, reasoning: true },
        ];
        let live = Ok(vec![
            ModelInfo {
                id: "gap".into(),
                label: "Gap".into(),
                vision: false,
                reasoning: None,
            },
            ModelInfo {
                id: "declared".into(),
                label: "Declared".into(),
                vision: false,
                reasoning: Some(false),
            },
            ModelInfo {
                id: "stranger".into(),
                label: "Stranger".into(),
                vision: false,
                reasoning: None,
            },
        ]);
        let (models, source) = resolve_model_list(live, &curated);
        assert_eq!(source, ModelSource::Live);
        assert_eq!(models[0].reasoning, Some(true), "gap filled from curated");
        assert_eq!(models[1].reasoning, Some(false), "live Some wins over curated");
        assert_eq!(models[2].reasoning, None, "uncurated id stays unknown");
    }

    /// Live smoke test against the keyless public OpenRouter catalog; run with
    /// `cargo test -- --ignored` when online.
    #[tokio::test]
    #[ignore = "hits the live OpenRouter API"]
    async fn openrouter_live_catalog_parses() {
        let provider = crate::providers::find_provider("openrouter").unwrap();
        let client = crate::http::build_client();
        let models = fetch_models(&client, provider, None).await.unwrap();
        assert!(models.len() > 100, "got {} models", models.len());
        assert!(models.iter().all(|m| !m.id.is_empty() && !m.label.is_empty()));
        // Roughly half of OpenRouter's catalog is vision-capable — at least one
        // must parse as such, proving `input_modalities` is being read.
        assert!(models.iter().any(|m| m.vision), "no vision models parsed");
    }
}

/// Offline wiremock tier: `fetch_models`' auth-header wiring and non-2xx
/// routing (`vault/2026-07-13_rust-http-mock-integration-tests.md`).
#[cfg(test)]
mod http_tests {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::test_support::mock_provider;

    #[tokio::test]
    async fn keyless_fetch_sends_no_auth_headers_and_parses() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &server.uri(),
            &format!("{}/models", server.uri()),
        );
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":"openai/gpt-4o-mini","name":"GPT-4o mini","architecture":{"input_modalities":["text","image"]}}]}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let models = fetch_models(&client, &provider, None).await.unwrap();
        assert_eq!(
            models,
            vec![ModelInfo {
                id: "openai/gpt-4o-mini".into(),
                label: "GPT-4o mini".into(),
                vision: true,
                reasoning: None,
            }]
        );

        // "Never send a key where it isn't needed": no auth header of either
        // protocol may be on the wire. Asserted on the recorded request —
        // wiremock has no absent-header matcher.
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].headers.get("authorization").is_none());
        assert!(requests[0].headers.get("x-api-key").is_none());
    }

    /// `fetch_models_at` is the runtime-endpoint core the Local AI catalog
    /// rides on: no registry entry, and Ollama's bare `/v1/models` payload
    /// (id-only rows) must parse — label falls back to the id, vision and
    /// reasoning stay unknown-conservative.
    #[tokio::test]
    async fn runtime_endpoint_fetch_parses_a_bare_ollama_payload() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"object":"list","data":[{"id":"llama3.2:3b","object":"model","created":1723800000,"owned_by":"library"}]}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let models = fetch_models_at(
            &client,
            ProviderKind::OpenAiCompatible,
            &format!("{}/v1/models", server.uri()),
            "Local",
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            models,
            vec![ModelInfo {
                id: "llama3.2:3b".into(),
                label: "llama3.2:3b".into(),
                vision: false,
                reasoning: None,
            }]
        );
    }

    #[tokio::test]
    async fn keyed_anthropic_fetch_sends_api_key_headers() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::Anthropic,
            &server.uri(),
            &format!("{}/models", server.uri()),
        );
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("x-api-key", "k"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":"claude-haiku-4-5-20251001","display_name":"Claude Haiku 4.5"}]}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let models = fetch_models(&client, &provider, Some("k")).await.unwrap();
        assert_eq!(models[0].label, "Claude Haiku 4.5");
        assert!(models[0].vision, "absent capabilities → default true");
    }

    #[tokio::test]
    async fn non_2xx_models_fetch_is_llm_error() {
        let server = MockServer::start().await;
        let provider = mock_provider(
            ProviderKind::OpenAiCompatible,
            &server.uri(),
            &format!("{}/models", server.uri()),
        );
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream down"))
            .mount(&server)
            .await;

        let client = crate::http::build_client();
        let err = fetch_models(&client, &provider, None).await.unwrap_err();
        match err {
            AppError::Llm {
                provider,
                status,
                body,
            } => {
                assert_eq!(provider, "MockProv");
                assert_eq!(status, 500);
                assert_eq!(body, "upstream down");
            }
            other => panic!("expected AppError::Llm, got {other:?}"),
        }
    }
}
