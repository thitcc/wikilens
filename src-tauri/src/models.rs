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

/// Fetch a provider's live model list. `api_key` is `None` for endpoints that
/// don't need auth (OpenRouter) — never send a key where it isn't needed.
pub async fn fetch_models(
    client: &reqwest::Client,
    provider: &Provider,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfo>, AppError> {
    let mut request = client.get(provider.models_endpoint).timeout(FETCH_TIMEOUT);
    if let Some(key) = api_key {
        request = match provider.kind {
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
            provider: provider.name,
            status: status.as_u16(),
            body: resp.text().await.unwrap_or_default(),
        });
    }
    let body = resp.text().await?;

    match provider.kind {
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
            ModelInfo {
                id: entry.id,
                label,
                vision,
            }
        })
        .collect())
}

/// Pick the list to serve: a live one when the fetch produced anything,
/// otherwise the provider's curated fallback. An *empty* live list also
/// degrades — an empty menu group would be strictly worse than a stale one.
pub fn resolve_model_list(
    live: Result<Vec<ModelInfo>, AppError>,
    curated: &[CuratedModel],
) -> (Vec<ModelInfo>, ModelSource) {
    match live {
        Ok(models) if !models.is_empty() => (models, ModelSource::Live),
        _ => (
            curated
                .iter()
                .map(|m| ModelInfo {
                    id: m.id.to_string(),
                    label: m.label.to_string(),
                    vision: m.vision,
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
                },
                ModelInfo {
                    id: "claude-haiku-4-5-20251001".into(),
                    label: "Claude Haiku 4.5".into(),
                    vision: true,
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
                vision: false, // no `architecture` in this fixture
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
        }]
    }

    #[test]
    fn resolve_prefers_non_empty_live_list() {
        let live = Ok(vec![ModelInfo {
            id: "live-model".into(),
            label: "Live Model".into(),
            vision: false,
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
            CuratedModel { id: "v", label: "V", vision: true },
            CuratedModel { id: "t", label: "T", vision: false },
        ];
        let (models, source) =
            resolve_model_list(Err(AppError::Parse("x".into())), &curated);
        assert_eq!(source, ModelSource::Fallback);
        assert!(models[0].vision);
        assert!(!models[1].vision);
    }

    /// Live smoke test against the keyless public OpenRouter catalog; run with
    /// `cargo test -- --ignored` when online.
    #[tokio::test]
    #[ignore = "hits the live OpenRouter API"]
    async fn openrouter_live_catalog_parses() {
        let provider = crate::providers::find_provider("openrouter").unwrap();
        let client = reqwest::Client::new();
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

        let client = reqwest::Client::new();
        let models = fetch_models(&client, &provider, None).await.unwrap();
        assert_eq!(
            models,
            vec![ModelInfo {
                id: "openai/gpt-4o-mini".into(),
                label: "GPT-4o mini".into(),
                vision: true,
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

        let client = reqwest::Client::new();
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

        let client = reqwest::Client::new();
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
