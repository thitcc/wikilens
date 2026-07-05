//! Static registry of supported LLM providers.
//!
//! Adding a provider = add one `Provider` entry to `PROVIDERS`. Behavior
//! differences between providers collapse to `ProviderKind`, so nothing else in
//! the codebase needs to change.

/// The wire protocol a provider speaks. Only two exist, so a closed enum keeps
/// request-building and SSE parsing to two `match` arms (see `llm.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    /// Anthropic Messages API: `x-api-key` header, top-level `system`,
    /// `content_block_delta` SSE events.
    Anthropic,
    /// OpenAI-compatible Chat Completions (DeepSeek, OpenRouter): `Bearer` auth,
    /// a `system` role message, `choices[0].delta.content` SSE with `[DONE]`.
    OpenAiCompatible,
}

/// A supported LLM provider, expressed as declarative data.
#[derive(Debug, Clone, Copy)]
pub struct Provider {
    /// Stable id used by the frontend and the `ask` command (e.g. "deepseek").
    pub id: &'static str,
    /// Human-readable name for the provider picker and error messages.
    pub name: &'static str,
    pub kind: ProviderKind,
    /// Full chat/messages endpoint URL.
    pub endpoint: &'static str,
    /// Env var holding this provider's API key.
    pub api_key_env: &'static str,
    /// Env var that overrides the model. Stored explicitly (rather than derived
    /// from `id`) so the exact name is greppable with no case-transform edge.
    pub model_env: &'static str,
    /// Model used when the override env var is unset.
    pub default_model: &'static str,
    /// Endpoint listing this provider's models (GET). See `models.rs`.
    pub models_endpoint: &'static str,
    /// Whether the models endpoint rejects keyless calls (Anthropic and
    /// DeepSeek 401 without a key; OpenRouter's catalog is public).
    pub models_need_key: bool,
    /// Tiny curated `(id, label)` fallback used when a live fetch isn't
    /// possible — a degraded mode, not a catalog. Ids verified against the
    /// live endpoints on 2026-07-05.
    pub curated_models: &'static [(&'static str, &'static str)],
    /// Extra static request headers (e.g. OpenRouter attribution); usually empty.
    pub extra_headers: &'static [(&'static str, &'static str)],
}

/// The supported providers. Anthropic is first, so it is the default selection.
pub static PROVIDERS: &[Provider] = &[
    Provider {
        id: "anthropic",
        name: "Anthropic",
        kind: ProviderKind::Anthropic,
        endpoint: "https://api.anthropic.com/v1/messages",
        api_key_env: "ANTHROPIC_API_KEY",
        model_env: "WIKILENS_ANTHROPIC_MODEL",
        default_model: "claude-haiku-4-5-20251001",
        models_endpoint: "https://api.anthropic.com/v1/models?limit=1000",
        models_need_key: true,
        curated_models: &[
            ("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
            ("claude-sonnet-5", "Claude Sonnet 5"),
            ("claude-opus-4-8", "Claude Opus 4.8"),
        ],
        extra_headers: &[],
    },
    Provider {
        id: "deepseek",
        name: "DeepSeek",
        kind: ProviderKind::OpenAiCompatible,
        endpoint: "https://api.deepseek.com/chat/completions",
        api_key_env: "DEEPSEEK_API_KEY",
        model_env: "WIKILENS_DEEPSEEK_MODEL",
        // Not the `deepseek-chat` alias — DeepSeek retires it on 2026-07-24.
        default_model: "deepseek-v4-flash",
        models_endpoint: "https://api.deepseek.com/models",
        models_need_key: true,
        curated_models: &[
            ("deepseek-v4-flash", "DeepSeek V4 Flash"),
            ("deepseek-v4-pro", "DeepSeek V4 Pro"),
        ],
        extra_headers: &[],
    },
    Provider {
        id: "openrouter",
        name: "OpenRouter",
        kind: ProviderKind::OpenAiCompatible,
        endpoint: "https://openrouter.ai/api/v1/chat/completions",
        api_key_env: "OPENROUTER_API_KEY",
        model_env: "WIKILENS_OPENROUTER_MODEL",
        default_model: "openai/gpt-4o-mini",
        models_endpoint: "https://openrouter.ai/api/v1/models",
        models_need_key: false,
        curated_models: &[
            ("openai/gpt-4o-mini", "GPT-4o mini"),
            ("anthropic/claude-sonnet-5", "Claude Sonnet 5"),
            ("google/gemini-2.5-flash", "Gemini 2.5 Flash"),
        ],
        // Optional attribution headers OpenRouter uses for its app leaderboard.
        // Harmless to send; ignored by other providers.
        extra_headers: &[
            ("HTTP-Referer", "https://wikilens.app"),
            ("X-Title", "WikiLens"),
        ],
    },
];

/// Provider selected when the UI hasn't chosen one yet or holds an unknown id.
pub const DEFAULT_PROVIDER_ID: &str = "anthropic";

/// Look up a provider by its `id`. Returns `None` for unknown ids.
pub fn find_provider(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// Read a trimmed, non-empty env var. `None` if unset or blank/whitespace-only.
fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

impl Provider {
    /// This provider's API key from its env var, or `None` if unset/blank.
    ///
    /// Kept as `Option` (like `wiki::games::find_game`) so this module stays free
    /// of `AppError`; `commands.rs` maps `None` to `AppError::MissingApiKey`.
    pub fn api_key(&self) -> Option<String> {
        env_nonempty(self.api_key_env)
    }

    /// The resolved model: the `WIKILENS_<PROVIDER>_MODEL` override if set,
    /// otherwise the built-in default.
    pub fn model(&self) -> String {
        resolve_model(env_nonempty(self.model_env), self.default_model)
    }

    /// Human label for a model id: the curated label when known, else the id
    /// itself. Live lists carry their own labels; this covers defaults and
    /// env overrides before any list is fetched.
    pub fn model_label<'a>(&self, id: &'a str) -> &'a str {
        self.curated_models
            .iter()
            .find(|(model_id, _)| *model_id == id)
            .map(|(_, label)| *label)
            .unwrap_or(id)
    }
}

/// Pure model-override resolution, split out so tests need no env mutation.
fn resolve_model(override_value: Option<String>, default: &str) -> String {
    override_value.unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_provider_known_and_unknown() {
        assert_eq!(find_provider("deepseek").map(|p| p.name), Some("DeepSeek"));
        assert!(find_provider("does-not-exist").is_none());
    }

    #[test]
    fn ids_are_unique() {
        for (i, a) in PROVIDERS.iter().enumerate() {
            for b in &PROVIDERS[i + 1..] {
                assert_ne!(a.id, b.id, "duplicate provider id: {}", a.id);
            }
        }
    }

    #[test]
    fn default_provider_exists() {
        assert!(find_provider(DEFAULT_PROVIDER_ID).is_some());
    }

    #[test]
    fn deepseek_and_openrouter_are_openai_compatible() {
        assert_eq!(
            find_provider("deepseek").map(|p| p.kind),
            Some(ProviderKind::OpenAiCompatible)
        );
        assert_eq!(
            find_provider("openrouter").map(|p| p.kind),
            Some(ProviderKind::OpenAiCompatible)
        );
        assert_eq!(
            find_provider("anthropic").map(|p| p.kind),
            Some(ProviderKind::Anthropic)
        );
    }

    #[test]
    fn model_override_takes_precedence_over_default() {
        assert_eq!(resolve_model(None, "deepseek-v4-flash"), "deepseek-v4-flash");
        assert_eq!(
            resolve_model(Some("deepseek-v4-pro".to_string()), "deepseek-v4-flash"),
            "deepseek-v4-pro"
        );
    }

    /// The curated list is the degraded-mode menu; the default must always be
    /// offered there, or a keyless/offline session shows a menu that can't
    /// even select what the app would use.
    #[test]
    fn default_model_is_in_curated_list() {
        for provider in PROVIDERS {
            assert!(
                provider
                    .curated_models
                    .iter()
                    .any(|(id, _)| *id == provider.default_model),
                "{}: default model {} missing from curated_models",
                provider.id,
                provider.default_model
            );
        }
    }

    #[test]
    fn curated_model_ids_are_unique_per_provider() {
        for provider in PROVIDERS {
            for (i, (a, _)) in provider.curated_models.iter().enumerate() {
                for (b, _) in &provider.curated_models[i + 1..] {
                    assert_ne!(a, b, "{}: duplicate curated model id {}", provider.id, a);
                }
            }
        }
    }

    #[test]
    fn model_label_uses_curated_label_or_falls_back_to_id() {
        let deepseek = find_provider("deepseek").unwrap();
        assert_eq!(deepseek.model_label("deepseek-v4-flash"), "DeepSeek V4 Flash");
        assert_eq!(deepseek.model_label("some-unknown-model"), "some-unknown-model");
    }
}
