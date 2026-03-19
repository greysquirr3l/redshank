//! Persistent settings: per-provider default model names and global reasoning effort.

use crate::domain::agent::{ProviderKind, ReasoningEffort};
use serde::{Deserialize, Serialize};

/// User-configurable persistent settings stored in `.redshank/settings.json`.
///
/// Tracks per-provider default model names and global default reasoning effort.
/// Unknown JSON keys are silently ignored on deserialization.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PersistentSettings {
    /// Fallback model name used when no per-provider default is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    /// Default reasoning effort applied to all requests unless overridden.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_effort: Option<ReasoningEffort>,
    /// Default model for Anthropic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model_anthropic: Option<String>,
    /// Default model for `OpenAI`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model_openai: Option<String>,
    /// Default model for `OpenRouter`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model_openrouter: Option<String>,
    /// Default model for Cerebras.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model_cerebras: Option<String>,
    /// Default model for Ollama.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model_ollama: Option<String>,
}

impl PersistentSettings {
    /// Get the default model name for a given provider kind.
    ///
    /// Returns the per-provider default if set, otherwise falls back to `default_model`.
    #[must_use]
    pub fn default_model_for_provider(&self, provider: ProviderKind) -> Option<&str> {
        let specific = match provider {
            ProviderKind::Anthropic => self.default_model_anthropic.as_deref(),
            ProviderKind::OpenAI => self.default_model_openai.as_deref(),
            ProviderKind::OpenRouter => self.default_model_openrouter.as_deref(),
            ProviderKind::Cerebras => self.default_model_cerebras.as_deref(),
            ProviderKind::Ollama => self.default_model_ollama.as_deref(),
        };
        specific.filter(|s| !s.trim().is_empty()).or_else(|| {
            self.default_model
                .as_deref()
                .filter(|s| !s.trim().is_empty())
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_all_none() {
        let s = PersistentSettings::default();
        assert!(s.default_model.is_none());
        assert!(s.default_reasoning_effort.is_none());
        assert!(s.default_model_anthropic.is_none());
    }

    #[test]
    fn per_provider_model_returns_specific_over_global() {
        let s = PersistentSettings {
            default_model: Some("global-model".into()),
            default_model_anthropic: Some("claude-sonnet-4-20250514".into()),
            ..Default::default()
        };
        assert_eq!(
            s.default_model_for_provider(ProviderKind::Anthropic),
            Some("claude-sonnet-4-20250514")
        );
        assert_eq!(
            s.default_model_for_provider(ProviderKind::OpenAI),
            Some("global-model")
        );
    }

    #[test]
    fn roundtrip_serde() {
        let s = PersistentSettings {
            default_model: Some("gpt-4o".into()),
            default_reasoning_effort: Some(ReasoningEffort::High),
            default_model_anthropic: Some("claude-sonnet-4-20250514".into()),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&s).unwrap();
        let restored: PersistentSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, restored);
    }

    #[test]
    fn unknown_keys_ignored() {
        let json = r#"{"default_model":"gpt-4o","unknown_field":"ignored"}"#;
        let s: PersistentSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.default_model.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn empty_string_model_treated_as_none() {
        let s = PersistentSettings {
            default_model_anthropic: Some("  ".into()),
            default_model: Some("fallback".into()),
            ..Default::default()
        };
        // Empty string should fall through to global
        assert_eq!(
            s.default_model_for_provider(ProviderKind::Anthropic),
            Some("fallback")
        );
    }
}
