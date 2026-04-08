//! Persistent settings: per-provider default model names, reasoning effort, and
//! data source configuration.

use crate::domain::agent::{ProviderKind, ReasoningEffort};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Fetcher / Data Source Configuration ──────────────────────────────────────

/// Per-source configuration for a data fetcher.
///
/// Each fetcher can be enabled/disabled and can have source-specific overrides
/// for rate limiting and pagination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FetcherSourceConfig {
    /// Whether this data source is enabled. Defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Source-specific rate limit override (milliseconds between requests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_ms: Option<u64>,
    /// Source-specific maximum pages override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pages: Option<u32>,
    /// Source-specific API key (alternative to credentials store).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

const fn default_enabled() -> bool {
    true
}

impl Default for FetcherSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rate_limit_ms: None,
            max_pages: None,
            api_key: None,
        }
    }
}

impl FetcherSourceConfig {
    /// Create a disabled source configuration.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create an enabled source configuration with custom rate limiting.
    #[must_use]
    pub fn with_rate_limit(rate_limit_ms: u64) -> Self {
        Self {
            enabled: true,
            rate_limit_ms: Some(rate_limit_ms),
            ..Default::default()
        }
    }
}

/// Map of fetcher source IDs to their configuration.
///
/// Source IDs are lowercase `snake_case` identifiers matching the module names
/// in `redshank-fetchers/src/fetchers/` (e.g., `"opencorporates"`, `"fec"`,
/// `"ofac_sdn"`).
pub type FetchersConfig = HashMap<String, FetcherSourceConfig>;

/// Known fetcher source identifiers.
///
/// This list is used to provide defaults and validate source IDs.
pub const KNOWN_FETCHERS: &[&str] = &[
    // T19: Core fetchers
    "census_acs",
    "epa_echo",
    "fdic",
    "fec",
    "icij_leaks",
    "ofac_sdn",
    "osha",
    "propublica_990",
    "sam_gov",
    "sec_edgar",
    "senate_lobbying",
    "usaspending",
    // T20: Extended fetchers
    "county_property",
    "courtlistener",
    "eu_sanctions",
    "federal_audit",
    "fincen_boi",
    "fpds",
    "gdelt",
    "gleif",
    "house_lobbying",
    "opencorporates",
    "state_sos",
    "un_sanctions",
    "wikidata",
    "world_bank_debarred",
    // T21: Individual-person OSINT fetchers
    "github_profile",
    "hibp",
    "social_profiles",
    "username_enum",
    "uspto",
    "voter_reg",
    "wayback",
    "whois_rdap",
];

// ── PersistentSettings ───────────────────────────────────────────────────────

/// User-configurable persistent settings stored in `.redshank/settings.json`.
///
/// Tracks per-provider default model names, global default reasoning effort,
/// and data source configuration. Unknown JSON keys are silently ignored on
/// deserialization.
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
    /// Per-source data fetcher configuration.
    ///
    /// Keys are fetcher source IDs (e.g., `"opencorporates"`, `"fec"`).
    /// Sources not listed in this map are enabled by default.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fetchers: FetchersConfig,
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

    /// Check if a data source fetcher is enabled.
    ///
    /// Sources not explicitly configured are enabled by default.
    #[must_use]
    pub fn is_fetcher_enabled(&self, source_id: &str) -> bool {
        self.fetchers.get(source_id).is_none_or(|cfg| cfg.enabled)
    }

    /// Get configuration for a specific data source fetcher.
    ///
    /// Returns `None` if no explicit configuration exists for this source
    /// (caller should use default settings).
    #[must_use]
    pub fn fetcher_config(&self, source_id: &str) -> Option<&FetcherSourceConfig> {
        self.fetchers.get(source_id)
    }

    /// Get all explicitly disabled fetcher source IDs.
    #[must_use]
    pub fn disabled_fetchers(&self) -> Vec<&str> {
        self.fetchers
            .iter()
            .filter(|(_, cfg)| !cfg.enabled)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Get all enabled fetcher source IDs from the known set.
    ///
    /// Returns known fetchers that are not explicitly disabled.
    #[must_use]
    pub fn enabled_known_fetchers(&self) -> Vec<&'static str> {
        KNOWN_FETCHERS
            .iter()
            .copied()
            .filter(|id| self.is_fetcher_enabled(id))
            .collect()
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

    // ── Fetcher configuration tests ──────────────────────────────────────────

    #[test]
    fn fetcher_source_config_default_is_enabled() {
        let cfg = FetcherSourceConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.rate_limit_ms.is_none());
        assert!(cfg.max_pages.is_none());
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn fetcher_source_config_disabled_helper() {
        let cfg = FetcherSourceConfig::disabled();
        assert!(!cfg.enabled);
    }

    #[test]
    fn fetcher_source_config_with_rate_limit() {
        let cfg = FetcherSourceConfig::with_rate_limit(1000);
        assert!(cfg.enabled);
        assert_eq!(cfg.rate_limit_ms, Some(1000));
    }

    #[test]
    fn unknown_fetcher_enabled_by_default() {
        let s = PersistentSettings::default();
        assert!(s.is_fetcher_enabled("opencorporates"));
        assert!(s.is_fetcher_enabled("nonexistent_source"));
    }

    #[test]
    fn explicitly_disabled_fetcher_returns_false() {
        let mut fetchers = HashMap::new();
        fetchers.insert("hibp".to_string(), FetcherSourceConfig::disabled());
        let s = PersistentSettings {
            fetchers,
            ..Default::default()
        };
        assert!(!s.is_fetcher_enabled("hibp"));
        assert!(s.is_fetcher_enabled("fec")); // not configured = enabled
    }

    #[test]
    fn fetcher_config_returns_explicit_config() {
        let mut fetchers = HashMap::new();
        fetchers.insert(
            "sec_edgar".to_string(),
            FetcherSourceConfig::with_rate_limit(2000),
        );
        let s = PersistentSettings {
            fetchers,
            ..Default::default()
        };
        let cfg = s.fetcher_config("sec_edgar").unwrap();
        assert_eq!(cfg.rate_limit_ms, Some(2000));
        assert!(s.fetcher_config("fec").is_none());
    }

    #[test]
    fn disabled_fetchers_returns_only_disabled() {
        let mut fetchers = HashMap::new();
        fetchers.insert("hibp".to_string(), FetcherSourceConfig::disabled());
        fetchers.insert("voter_reg".to_string(), FetcherSourceConfig::disabled());
        fetchers.insert("fec".to_string(), FetcherSourceConfig::default()); // enabled
        let s = PersistentSettings {
            fetchers,
            ..Default::default()
        };
        let disabled = s.disabled_fetchers();
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains(&"hibp"));
        assert!(disabled.contains(&"voter_reg"));
    }

    #[test]
    fn enabled_known_fetchers_excludes_disabled() {
        let mut fetchers = HashMap::new();
        fetchers.insert("hibp".to_string(), FetcherSourceConfig::disabled());
        let s = PersistentSettings {
            fetchers,
            ..Default::default()
        };
        let enabled = s.enabled_known_fetchers();
        assert!(!enabled.contains(&"hibp"));
        assert!(enabled.contains(&"fec"));
        assert!(enabled.contains(&"opencorporates"));
    }

    #[test]
    fn fetchers_roundtrip_serde() {
        let mut fetchers = HashMap::new();
        fetchers.insert("hibp".to_string(), FetcherSourceConfig::disabled());
        fetchers.insert(
            "sec_edgar".to_string(),
            FetcherSourceConfig {
                enabled: true,
                rate_limit_ms: Some(1500),
                max_pages: Some(50),
                api_key: None,
            },
        );
        let s = PersistentSettings {
            fetchers,
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&s).unwrap();
        let restored: PersistentSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, restored);
        assert!(!restored.is_fetcher_enabled("hibp"));
        assert_eq!(
            restored.fetcher_config("sec_edgar").unwrap().rate_limit_ms,
            Some(1500)
        );
    }

    #[test]
    fn fetcher_config_json_parsing() {
        let json = r#"{
            "fetchers": {
                "hibp": { "enabled": false },
                "fec": { "enabled": true, "rate_limit_ms": 1000 }
            }
        }"#;
        let s: PersistentSettings = serde_json::from_str(json).unwrap();
        assert!(!s.is_fetcher_enabled("hibp"));
        assert!(s.is_fetcher_enabled("fec"));
        assert_eq!(s.fetcher_config("fec").unwrap().rate_limit_ms, Some(1000));
    }

    #[test]
    fn known_fetchers_list_not_empty() {
        assert!(!KNOWN_FETCHERS.is_empty());
        assert!(KNOWN_FETCHERS.contains(&"opencorporates"));
        assert!(KNOWN_FETCHERS.contains(&"ofac_sdn"));
    }
}
