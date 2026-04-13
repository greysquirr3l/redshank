//! Provider builder: constructs the correct [`ModelProvider`] implementation
//! from an [`AgentConfig`] and [`CredentialBundle`].
//!
//! Because [`ModelProvider`] uses RPITIT it is **not** dyn-compatible.
//! Instead we use a [`ProviderBox`] enum that wraps each concrete provider
//! and delegates through match arms.

#[cfg(feature = "runtime")]
use reqwest::Client;

use crate::domain::agent::{AgentConfig, ProviderKind};
use crate::domain::credentials::{CredentialBundle, CredentialGuard};
use crate::domain::errors::DomainError;
use crate::domain::session::ModelTurn;
use crate::domain::settings::{
    PersistentSettings, ProviderDeploymentKind, ProviderEndpointConfig, ProviderProtocolKind,
};
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
    /// The provider is explicitly disabled in persistent settings.
    #[error("provider {provider:?} is disabled in settings")]
    ProviderDisabled {
        /// Disabled provider kind.
        provider: ProviderKind,
    },
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
    build_provider_with_settings(config, &PersistentSettings::default(), creds)
}

/// Build a [`ProviderBox`] using persistent provider endpoint settings.
///
/// # Errors
///
/// Returns `Err` if the provider is disabled or required credentials are missing.
pub fn build_provider_with_settings(
    config: &AgentConfig,
    settings: &PersistentSettings,
    creds: &CredentialBundle,
) -> Result<ProviderBox, BuildError> {
    let effort = Some(config.reasoning_effort);
    let endpoint = effective_provider_endpoint(config.provider, settings, creds)?;
    let model_name = resolved_model_name(config, settings);
    match config.provider {
        ProviderKind::Anthropic => {
            let key = resolve_provider_key(config.provider, &endpoint, creds)?;
            let mut provider = AnthropicModel::new(key, model_name, effort);
            if let Some(base_url) = endpoint.base_url.as_deref() {
                provider = provider.with_base_url(base_url);
            }
            Ok(ProviderBox::Anthropic(provider))
        }
        kind @ (ProviderKind::OpenAI
        | ProviderKind::OpenRouter
        | ProviderKind::Cerebras
        | ProviderKind::Ollama) => {
            let key = resolve_provider_key(kind, &endpoint, creds)?;
            let mut provider = OpenAICompatibleModel::for_provider(kind, key, model_name, effort);
            if let Some(base_url) = endpoint.base_url.as_deref() {
                provider = provider.with_base_url(base_url);
            }
            Ok(ProviderBox::OpenAICompat(provider))
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
    list_models_with_settings(kind, &PersistentSettings::default(), creds).await
}

/// List available models using settings-aware provider endpoint configuration.
///
/// # Errors
///
/// Returns `Err` if the provider is disabled, auth is missing, or the endpoint fails.
#[allow(clippy::too_many_lines)]
#[cfg(feature = "runtime")]
pub async fn list_models_with_settings(
    kind: ProviderKind,
    settings: &PersistentSettings,
    creds: &CredentialBundle,
) -> Result<Vec<String>, DomainError> {
    let endpoint = effective_provider_endpoint(kind, settings, creds)
        .map_err(|err| DomainError::Validation(err.to_string()))?;
    let key = resolve_provider_key(kind, &endpoint, creds)
        .map_err(|err| DomainError::Validation(err.to_string()))?;

    let (url, auth_header, auth_value) = match kind {
        ProviderKind::Anthropic => (
            format!(
                "{}/models",
                endpoint
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.anthropic.com/v1")
                    .trim_end_matches('/')
            ),
            "x-api-key".to_string(),
            key.expose().clone(),
        ),
        ProviderKind::OpenAI => (
            format!(
                "{}/models",
                endpoint
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.openai.com/v1")
                    .trim_end_matches('/')
            ),
            "Authorization".to_string(),
            format!("Bearer {}", key.expose()),
        ),
        ProviderKind::OpenRouter => (
            format!(
                "{}/models",
                endpoint
                    .base_url
                    .as_deref()
                    .unwrap_or("https://openrouter.ai/api/v1")
                    .trim_end_matches('/')
            ),
            "Authorization".to_string(),
            format!("Bearer {}", key.expose()),
        ),
        ProviderKind::Cerebras => (
            format!(
                "{}/models",
                endpoint
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.cerebras.ai/v1")
                    .trim_end_matches('/')
            ),
            "Authorization".to_string(),
            format!("Bearer {}", key.expose()),
        ),
        ProviderKind::Ollama => {
            let base = ollama_root_base_url(&endpoint, creds);
            (
                format!("{}/api/tags", base.trim_end_matches('/')),
                String::new(),
                String::new(),
            )
        }
    };

    let client = Client::new();
    let mut req = client.get(&url);
    if !auth_header.is_empty() && !auth_value.is_empty() {
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
) -> Result<CredentialGuard<String>, BuildError> {
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

fn effective_provider_endpoint(
    provider: ProviderKind,
    settings: &PersistentSettings,
    creds: &CredentialBundle,
) -> Result<ProviderEndpointConfig, BuildError> {
    let mut endpoint = ProviderEndpointConfig {
        enabled: true,
        protocol: default_protocol_for(provider),
        base_url: Some(default_base_url_for(provider, creds)),
        default_model: settings
            .default_model_for_provider(provider)
            .map(str::to_string),
        credential_field_name: default_credential_field_name(provider).map(str::to_string),
        deployment: default_deployment_for(provider),
    };

    if let Some(explicit) = settings.provider_endpoint(provider) {
        endpoint.enabled = explicit.enabled;
        endpoint.protocol = explicit.protocol;
        endpoint.deployment = explicit.deployment;
        if explicit.base_url.is_some() {
            endpoint.base_url.clone_from(&explicit.base_url);
        }
        if explicit.default_model.is_some() {
            endpoint.default_model.clone_from(&explicit.default_model);
        }
        if explicit.credential_field_name.is_some() || explicit.allows_anonymous_access() {
            endpoint
                .credential_field_name
                .clone_from(&explicit.credential_field_name);
        }
    }

    if !endpoint.enabled {
        return Err(BuildError::ProviderDisabled { provider });
    }

    Ok(endpoint)
}

fn resolved_model_name(config: &AgentConfig, settings: &PersistentSettings) -> String {
    if !config.model.trim().is_empty() {
        return config.model.clone();
    }

    settings
        .default_model_for_provider(config.provider)
        .map_or_else(String::new, str::to_string)
}

const fn default_protocol_for(provider: ProviderKind) -> ProviderProtocolKind {
    match provider {
        ProviderKind::Anthropic => ProviderProtocolKind::Native,
        ProviderKind::OpenAI
        | ProviderKind::OpenRouter
        | ProviderKind::Cerebras
        | ProviderKind::Ollama => ProviderProtocolKind::OpenAiCompatible,
    }
}

const fn default_deployment_for(provider: ProviderKind) -> ProviderDeploymentKind {
    match provider {
        ProviderKind::Ollama => ProviderDeploymentKind::Local,
        ProviderKind::Anthropic
        | ProviderKind::OpenAI
        | ProviderKind::OpenRouter
        | ProviderKind::Cerebras => ProviderDeploymentKind::Hosted,
    }
}

const fn default_credential_field_name(provider: ProviderKind) -> Option<&'static str> {
    match provider {
        ProviderKind::Anthropic => Some("anthropic_api_key"),
        ProviderKind::OpenAI => Some("openai_api_key"),
        ProviderKind::OpenRouter => Some("openrouter_api_key"),
        ProviderKind::Cerebras => Some("cerebras_api_key"),
        ProviderKind::Ollama => None,
    }
}

fn default_base_url_for(provider: ProviderKind, creds: &CredentialBundle) -> String {
    match provider {
        ProviderKind::Anthropic => "https://api.anthropic.com".to_string(),
        ProviderKind::OpenAI => "https://api.openai.com/v1".to_string(),
        ProviderKind::OpenRouter => "https://openrouter.ai/api/v1".to_string(),
        ProviderKind::Cerebras => "https://api.cerebras.ai/v1".to_string(),
        ProviderKind::Ollama => normalize_ollama_chat_base_url(
            creds
                .ollama_base_url
                .as_deref()
                .unwrap_or("http://localhost:11434"),
        ),
    }
}

fn normalize_ollama_chat_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}

fn ollama_root_base_url(endpoint: &ProviderEndpointConfig, creds: &CredentialBundle) -> String {
    let base = endpoint.base_url.as_deref().unwrap_or_else(|| {
        creds
            .ollama_base_url
            .as_deref()
            .unwrap_or("http://localhost:11434")
    });
    base.trim_end_matches("/v1")
        .trim_end_matches('/')
        .to_string()
}

fn resolve_provider_key(
    provider: ProviderKind,
    endpoint: &ProviderEndpointConfig,
    creds: &CredentialBundle,
) -> Result<CredentialGuard<String>, BuildError> {
    if endpoint.allows_anonymous_access() {
        return Ok(CredentialGuard::new(String::new()));
    }

    if let Some(field_name) = endpoint.credential_field_name.as_deref() {
        return credential_by_name(creds, field_name).ok_or(BuildError::MissingApiKey { provider });
    }

    api_key_for(provider, creds)
}

fn credential_by_name(
    creds: &CredentialBundle,
    field_name: &str,
) -> Option<CredentialGuard<String>> {
    match field_name {
        "anthropic_api_key" => creds.anthropic_api_key.clone(),
        "openai_api_key" => creds.openai_api_key.clone(),
        "openrouter_api_key" => creds.openrouter_api_key.clone(),
        "cerebras_api_key" => creds.cerebras_api_key.clone(),
        "github_token" => creds.github_token.clone(),
        _ => None,
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::credentials::CredentialGuard;
    use std::collections::HashMap;

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

    #[test]
    fn build_provider_with_settings_uses_local_openai_compatible_endpoint() {
        let settings = PersistentSettings {
            providers: HashMap::from([(
                ProviderKind::OpenAI,
                ProviderEndpointConfig {
                    enabled: true,
                    protocol: ProviderProtocolKind::OpenAiCompatible,
                    base_url: Some("http://localhost:1234/v1".to_string()),
                    default_model: Some("qwen2.5-coder".to_string()),
                    credential_field_name: None,
                    deployment: ProviderDeploymentKind::Local,
                },
            )]),
            ..PersistentSettings::default()
        };
        let config = AgentConfig {
            provider: ProviderKind::OpenAI,
            model: String::new(),
            ..AgentConfig::default()
        };

        let provider =
            build_provider_with_settings(&config, &settings, &CredentialBundle::default()).unwrap();

        assert!(matches!(provider, ProviderBox::OpenAICompat(_)));
        if let ProviderBox::OpenAICompat(model) = provider {
            assert_eq!(model.base_url(), "http://localhost:1234/v1");
            assert_eq!(model.model_name(), "qwen2.5-coder");
        }
    }

    #[test]
    fn build_provider_with_settings_uses_named_credential_reference() {
        let settings = PersistentSettings {
            providers: HashMap::from([(
                ProviderKind::OpenAI,
                ProviderEndpointConfig {
                    enabled: true,
                    protocol: ProviderProtocolKind::OpenAiCompatible,
                    base_url: Some("https://gateway.example/v1".to_string()),
                    default_model: Some("gpt-4.1-mini".to_string()),
                    credential_field_name: Some("github_token".to_string()),
                    deployment: ProviderDeploymentKind::Hosted,
                },
            )]),
            ..PersistentSettings::default()
        };
        let creds = CredentialBundle {
            github_token: Some(CredentialGuard::new("ghp-test".to_string())),
            ..CredentialBundle::default()
        };
        let config = AgentConfig {
            provider: ProviderKind::OpenAI,
            model: String::new(),
            ..AgentConfig::default()
        };

        let provider = build_provider_with_settings(&config, &settings, &creds).unwrap();

        assert!(matches!(provider, ProviderBox::OpenAICompat(_)));
        if let ProviderBox::OpenAICompat(model) = provider {
            assert_eq!(model.base_url(), "https://gateway.example/v1");
            assert_eq!(model.model_name(), "gpt-4.1-mini");
        }
    }

    #[test]
    fn disabled_provider_endpoint_errors() {
        let settings = PersistentSettings {
            providers: HashMap::from([(
                ProviderKind::Anthropic,
                ProviderEndpointConfig {
                    enabled: false,
                    protocol: ProviderProtocolKind::Native,
                    base_url: None,
                    default_model: None,
                    credential_field_name: Some("anthropic_api_key".to_string()),
                    deployment: ProviderDeploymentKind::Hosted,
                },
            )]),
            ..PersistentSettings::default()
        };
        let config = AgentConfig {
            provider: ProviderKind::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
            ..AgentConfig::default()
        };

        let err = build_provider_with_settings(&config, &settings, &CredentialBundle::default())
            .unwrap_err();

        assert!(matches!(
            err,
            BuildError::ProviderDisabled {
                provider: ProviderKind::Anthropic
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
