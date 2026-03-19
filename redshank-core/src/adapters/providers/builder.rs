//! Provider builder: constructs the correct [`ModelProvider`] implementation
//! from an [`AgentConfig`] and [`CredentialBundle`].
//!
//! Because [`ModelProvider`] uses RPITIT it is **not** dyn-compatible.
//! Instead we use a [`ProviderBox`] enum that wraps each concrete provider
//! and delegates through match arms.

#[cfg(feature = "runtime")]
use reqwest::Client;

use crate::domain::agent::{AgentConfig, ProviderKind};
use crate::domain::credentials::CredentialBundle;
use crate::domain::errors::DomainError;
use crate::domain::session::ModelTurn;
use crate::ports::model_provider::{ChatMessage, ModelProvider, ToolDefinition};

use super::anthropic::AnthropicModel;
use super::openai_compat::OpenAICompatibleModel;

// ── ProviderBox ─────────────────────────────────────────────

/// Enum-dispatch wrapper around concrete provider implementations.
///
/// Because [`ModelProvider`] uses RPITIT, it cannot be used as `dyn ModelProvider`.
/// `ProviderBox` achieves the same runtime polymorphism via exhaustive match.
#[derive(Debug)]
pub enum ProviderBox {
    /// Anthropic native Messages API provider.
    Anthropic(AnthropicModel),
    /// OpenAI-compatible provider (`OpenAI`, `OpenRouter`, Cerebras, Ollama).
    OpenAICompat(OpenAICompatibleModel),
}

impl ModelProvider for ProviderBox {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ModelTurn, DomainError> {
        match self {
            Self::Anthropic(p) => p.complete(messages, tools).await,
            Self::OpenAICompat(p) => p.complete(messages, tools).await,
        }
    }

    fn count_tokens(&self, messages: &[ChatMessage]) -> Result<u32, DomainError> {
        match self {
            Self::Anthropic(p) => p.count_tokens(messages),
            Self::OpenAICompat(p) => p.count_tokens(messages),
        }
    }

    fn context_window(&self) -> u64 {
        match self {
            Self::Anthropic(p) => p.context_window(),
            Self::OpenAICompat(p) => p.context_window(),
        }
    }

    fn model_name(&self) -> &str {
        match self {
            Self::Anthropic(p) => p.model_name(),
            Self::OpenAICompat(p) => p.model_name(),
        }
    }
}

// ── Builder Error ───────────────────────────────────────────

/// Errors that can occur when building a provider.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// The required API key for the inferred provider is missing.
    #[error("missing API key for provider {provider:?}")]
    MissingApiKey {
        /// The provider that requires the missing key.
        provider: ProviderKind,
    },
    /// Could not infer a provider from the model name.
    #[error("cannot infer provider from model name: {model}")]
    UnknownModel {
        /// The model name that could not be matched.
        model: String,
    },
}

// ── Public builder functions ────────────────────────────────

/// Infer [`ProviderKind`] from a model name.
///
/// Delegates to [`ProviderKind::from_model_name`] and converts `None`
/// to a [`BuildError::UnknownModel`].
///
/// # Errors
///
/// Returns `Err(BuildError::UnknownModel)` if the model name is unrecognised.
pub fn infer_provider(model: &str) -> Result<ProviderKind, BuildError> {
    ProviderKind::from_model_name(model).ok_or_else(|| BuildError::UnknownModel {
        model: model.to_string(),
    })
}

/// Build a [`ProviderBox`] from an [`AgentConfig`] and [`CredentialBundle`].
///
/// The provider kind is taken from `config.provider`, and the required API key
/// is extracted from the credential bundle.
///
/// # Errors
///
/// Returns `Err(BuildError::MissingApiKey)` if the required API key is absent.
pub fn build_provider(
    config: &AgentConfig,
    creds: &CredentialBundle,
) -> Result<ProviderBox, BuildError> {
    let effort = Some(config.reasoning_effort);
    match config.provider {
        ProviderKind::Anthropic => {
            let key = creds
                .anthropic_api_key
                .clone()
                .ok_or(BuildError::MissingApiKey {
                    provider: ProviderKind::Anthropic,
                })?;
            Ok(ProviderBox::Anthropic(AnthropicModel::new(
                key,
                config.model.clone(),
                effort,
            )))
        }
        kind @ (ProviderKind::OpenAI
        | ProviderKind::OpenRouter
        | ProviderKind::Cerebras
        | ProviderKind::Ollama) => {
            let key = api_key_for(kind, creds)?;
            Ok(ProviderBox::OpenAICompat(
                OpenAICompatibleModel::for_provider(kind, key, config.model.clone(), effort),
            ))
        }
    }
}

/// Build a cheap "judge" model for acceptance-criteria evaluation.
///
/// Tries to build `claude-haiku-4-5` (Anthropic) first,
/// then falls back to `gpt-4o-mini` (`OpenAI`). Returns [`BuildError::MissingApiKey`]
/// if neither key is available.
///
/// # Errors
///
/// Returns `Err(BuildError::MissingApiKey)` if neither Anthropic nor `OpenAI` keys are present.
pub fn build_judge_model(creds: &CredentialBundle) -> Result<ProviderBox, BuildError> {
    // Prefer Anthropic haiku for judge
    if let Some(key) = creds.anthropic_api_key.clone() {
        return Ok(ProviderBox::Anthropic(AnthropicModel::new(
            key,
            "claude-haiku-4-5-20241022".to_string(),
            None,
        )));
    }
    // Fall back to OpenAI mini
    if let Some(key) = creds.openai_api_key.clone() {
        return Ok(ProviderBox::OpenAICompat(
            OpenAICompatibleModel::for_provider(
                ProviderKind::OpenAI,
                key,
                "gpt-4o-mini".to_string(),
                None,
            ),
        ));
    }
    Err(BuildError::MissingApiKey {
        provider: ProviderKind::Anthropic,
    })
}

/// List available models from a provider's API.
///
/// Makes a GET request to the provider's `/models` endpoint and
/// returns a list of model identifier strings.
///
/// # Errors
///
/// Returns `Err` if the API key is missing, the HTTP request fails, or the
/// response cannot be parsed.
// Each provider arm has distinct URL, auth-header, and pagination logic; extraction would reduce clarity.
#[allow(clippy::too_many_lines)]
#[cfg(feature = "runtime")]
pub async fn list_models(
    kind: ProviderKind,
    creds: &CredentialBundle,
) -> Result<Vec<String>, DomainError> {
    let (url, auth_header, auth_value) = match kind {
        ProviderKind::Anthropic => {
            let key = creds
                .anthropic_api_key
                .as_ref()
                .ok_or_else(|| DomainError::Validation("missing Anthropic API key".into()))?;
            (
                "https://api.anthropic.com/v1/models".to_string(),
                "x-api-key".to_string(),
                key.expose().clone(),
            )
        }
        ProviderKind::OpenAI => {
            let key = creds
                .openai_api_key
                .as_ref()
                .ok_or_else(|| DomainError::Validation("missing OpenAI API key".into()))?;
            (
                "https://api.openai.com/v1/models".to_string(),
                "Authorization".to_string(),
                format!("Bearer {}", key.expose()),
            )
        }
        ProviderKind::OpenRouter => {
            let key = creds
                .openrouter_api_key
                .as_ref()
                .ok_or_else(|| DomainError::Validation("missing OpenRouter API key".into()))?;
            (
                "https://openrouter.ai/api/v1/models".to_string(),
                "Authorization".to_string(),
                format!("Bearer {}", key.expose()),
            )
        }
        ProviderKind::Cerebras => {
            let key = creds
                .cerebras_api_key
                .as_ref()
                .ok_or_else(|| DomainError::Validation("missing Cerebras API key".into()))?;
            (
                "https://api.cerebras.ai/v1/models".to_string(),
                "Authorization".to_string(),
                format!("Bearer {}", key.expose()),
            )
        }
        ProviderKind::Ollama => {
            let base = creds
                .ollama_base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            (format!("{base}/api/tags"), String::new(), String::new())
        }
    };

    let client = Client::new();
    let mut req = client.get(&url);
    if !auth_header.is_empty() {
        req = req.header(&auth_header, &auth_value);
    }
    if kind == ProviderKind::Anthropic {
        req = req.header("anthropic-version", "2023-06-01");
    }

    let resp = req
        .send()
        .await
        .map_err(|e| DomainError::Other(format!("list_models request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(DomainError::Other(format!(
            "list_models returned {status}: {body}"
        )));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DomainError::Other(format!("list_models JSON parse failed: {e}")))?;

    // Ollama uses { "models": [...] } with "name" field
    // OpenAI/Anthropic use { "data": [...] } with "id" field
    let models = if kind == ProviderKind::Ollama {
        json.get("models")
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        m.get("name")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        json.get("data")
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        m.get("id")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    Ok(models)
}

// ── Helpers ─────────────────────────────────────────────────

/// Extract the API key for a given OpenAI-compatible provider from the credential bundle.
fn api_key_for(
    kind: ProviderKind,
    creds: &CredentialBundle,
) -> Result<crate::domain::credentials::CredentialGuard<String>, BuildError> {
    let key = match kind {
        ProviderKind::OpenAI => creds.openai_api_key.clone(),
        ProviderKind::OpenRouter => creds.openrouter_api_key.clone(),
        ProviderKind::Cerebras => creds.cerebras_api_key.clone(),
        ProviderKind::Ollama => {
            // Ollama doesn't require an API key; use a placeholder.
            Some(crate::domain::credentials::CredentialGuard::new(
                String::new(),
            ))
        }
        ProviderKind::Anthropic => creds.anthropic_api_key.clone(),
    };
    key.ok_or(BuildError::MissingApiKey { provider: kind })
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::credentials::CredentialGuard;

    fn make_creds(provider: ProviderKind) -> CredentialBundle {
        let mut creds = CredentialBundle::default();
        match provider {
            ProviderKind::Anthropic => {
                creds.anthropic_api_key = Some(CredentialGuard::new("sk-ant-test".to_string()));
            }
            ProviderKind::OpenAI => {
                creds.openai_api_key = Some(CredentialGuard::new("sk-openai-test".to_string()));
            }
            ProviderKind::OpenRouter => {
                creds.openrouter_api_key = Some(CredentialGuard::new("sk-or-test".to_string()));
            }
            ProviderKind::Cerebras => {
                creds.cerebras_api_key = Some(CredentialGuard::new("csk-test".to_string()));
            }
            ProviderKind::Ollama => {
                // no key required
            }
        }
        creds
    }

    // ── infer_provider ──────────────────────────────────────

    #[test]
    fn infer_claude_is_anthropic() {
        assert_eq!(
            infer_provider("claude-opus-4-6").unwrap(),
            ProviderKind::Anthropic
        );
    }

    #[test]
    fn infer_claude_sonnet_is_anthropic() {
        assert_eq!(
            infer_provider("claude-sonnet-4-20250514").unwrap(),
            ProviderKind::Anthropic
        );
    }

    #[test]
    fn infer_gpt_is_openai() {
        assert_eq!(infer_provider("gpt-5.2").unwrap(), ProviderKind::OpenAI);
    }

    #[test]
    fn infer_o1_is_openai() {
        assert_eq!(infer_provider("o1-preview").unwrap(), ProviderKind::OpenAI);
    }

    #[test]
    fn infer_o3_is_openai() {
        assert_eq!(infer_provider("o3-mini").unwrap(), ProviderKind::OpenAI);
    }

    #[test]
    fn infer_o4_is_openai() {
        assert_eq!(infer_provider("o4-mini").unwrap(), ProviderKind::OpenAI);
    }

    #[test]
    fn infer_ollama_prefix() {
        assert_eq!(
            infer_provider("ollama/llama3").unwrap(),
            ProviderKind::Ollama
        );
    }

    #[test]
    fn infer_cerebras_prefix() {
        assert_eq!(
            infer_provider("cerebras/llama3.3-70b").unwrap(),
            ProviderKind::Cerebras
        );
    }

    #[test]
    fn infer_llama_is_cerebras() {
        assert_eq!(
            infer_provider("llama3.3-70b").unwrap(),
            ProviderKind::Cerebras
        );
    }

    #[test]
    fn infer_openrouter_slash() {
        assert_eq!(
            infer_provider("meta-llama/llama-3-70b").unwrap(),
            ProviderKind::OpenRouter
        );
    }

    #[test]
    fn infer_unknown_model_fails() {
        let err = infer_provider("unknown-model-xyz").unwrap_err();
        assert!(matches!(err, BuildError::UnknownModel { .. }));
    }

    // ── build_provider ──────────────────────────────────────

    #[test]
    fn build_anthropic_provider() {
        let creds = make_creds(ProviderKind::Anthropic);
        let config = AgentConfig {
            provider: ProviderKind::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        assert!(matches!(p, ProviderBox::Anthropic(_)));
        assert_eq!(p.model_name(), "claude-sonnet-4-20250514");
    }

    #[test]
    fn build_openai_provider() {
        let creds = make_creds(ProviderKind::OpenAI);
        let config = AgentConfig {
            provider: ProviderKind::OpenAI,
            model: "gpt-5.2".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        assert!(matches!(p, ProviderBox::OpenAICompat(_)));
        assert_eq!(p.model_name(), "gpt-5.2");
    }

    #[test]
    fn build_openrouter_provider() {
        let creds = make_creds(ProviderKind::OpenRouter);
        let config = AgentConfig {
            provider: ProviderKind::OpenRouter,
            model: "meta-llama/llama-3-70b".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        assert!(matches!(p, ProviderBox::OpenAICompat(_)));
    }

    #[test]
    fn build_cerebras_provider() {
        let creds = make_creds(ProviderKind::Cerebras);
        let config = AgentConfig {
            provider: ProviderKind::Cerebras,
            model: "llama3.3-70b".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        assert!(matches!(p, ProviderBox::OpenAICompat(_)));
    }

    #[test]
    fn build_ollama_provider() {
        let creds = make_creds(ProviderKind::Ollama);
        let config = AgentConfig {
            provider: ProviderKind::Ollama,
            model: "ollama/llama3".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        assert!(matches!(p, ProviderBox::OpenAICompat(_)));
    }

    #[test]
    fn build_provider_missing_key_errors() {
        let creds = CredentialBundle::default(); // no keys
        let config = AgentConfig {
            provider: ProviderKind::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            ..AgentConfig::default()
        };
        let err = build_provider(&config, &creds).unwrap_err();
        assert!(matches!(
            err,
            BuildError::MissingApiKey {
                provider: ProviderKind::Anthropic
            }
        ));
    }

    #[test]
    fn build_openai_missing_key_errors() {
        let creds = CredentialBundle::default();
        let config = AgentConfig {
            provider: ProviderKind::OpenAI,
            model: "gpt-5.2".to_string(),
            ..AgentConfig::default()
        };
        let err = build_provider(&config, &creds).unwrap_err();
        assert!(matches!(
            err,
            BuildError::MissingApiKey {
                provider: ProviderKind::OpenAI
            }
        ));
    }

    // ── build_judge_model ───────────────────────────────────

    #[test]
    fn judge_prefers_anthropic_haiku() {
        let creds = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("sk-ant-test".to_string())),
            openai_api_key: Some(CredentialGuard::new("sk-openai-test".to_string())),
            ..Default::default()
        };
        let p = build_judge_model(&creds).unwrap();
        assert!(matches!(p, ProviderBox::Anthropic(_)));
        assert_eq!(p.model_name(), "claude-haiku-4-5-20241022");
    }

    #[test]
    fn judge_falls_back_to_openai() {
        let creds = CredentialBundle {
            openai_api_key: Some(CredentialGuard::new("sk-openai-test".to_string())),
            ..Default::default()
        };
        let p = build_judge_model(&creds).unwrap();
        assert!(matches!(p, ProviderBox::OpenAICompat(_)));
        assert_eq!(p.model_name(), "gpt-4o-mini");
    }

    #[test]
    fn judge_no_keys_errors() {
        let creds = CredentialBundle::default();
        let err = build_judge_model(&creds).unwrap_err();
        assert!(matches!(err, BuildError::MissingApiKey { .. }));
    }

    // ── ProviderBox delegates ───────────────────────────────

    #[test]
    fn provider_box_context_window() {
        let creds = make_creds(ProviderKind::Anthropic);
        let config = AgentConfig {
            provider: ProviderKind::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        assert_eq!(p.context_window(), 200_000);
    }

    #[test]
    fn provider_box_count_tokens() {
        let creds = make_creds(ProviderKind::OpenAI);
        let config = AgentConfig {
            provider: ProviderKind::OpenAI,
            model: "gpt-5.2".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        let msgs = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello world".to_string(),
            tool_calls: vec![],
            tool_call_id: None,
        }];
        let count = p.count_tokens(&msgs).unwrap();
        assert!(count > 0);
    }

    // ── Debug impls ─────────────────────────────────────────

    #[test]
    fn provider_box_debug_no_secrets() {
        let creds = make_creds(ProviderKind::Anthropic);
        let config = AgentConfig {
            provider: ProviderKind::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            ..AgentConfig::default()
        };
        let p = build_provider(&config, &creds).unwrap();
        let debug = format!("{p:?}");
        assert!(!debug.contains("sk-ant-test"));
        assert!(debug.contains("claude-sonnet"));
    }
}
