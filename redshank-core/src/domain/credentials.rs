//! Credential bundle and `CredentialGuard<T>` newtype.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

/// A credential value that masks its contents in Debug and Display output.
///
/// `Debug` and `Display` always print `"***REDACTED***"` regardless of inner value.
/// `Deref` provides transparent access to the inner value where needed.
/// `Serialize` delegates to the inner type; `Deserialize` wraps the inner type.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CredentialGuard<T>(T);

impl<T> CredentialGuard<T> {
    /// Wrap a value in a credential guard.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Access the inner value. Only use where the credential is actually needed.
    pub const fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> Deref for CredentialGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> fmt::Debug for CredentialGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***REDACTED***")
    }
}

impl<T> fmt::Display for CredentialGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***REDACTED***")
    }
}

impl<T: PartialEq> PartialEq for CredentialGuard<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Eq + PartialEq> Eq for CredentialGuard<T> {}

/// Bundle of API keys and credentials resolved from multiple sources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialBundle {
    /// `OpenAI` API key.
    pub openai_api_key: Option<CredentialGuard<String>>,
    /// Anthropic API key.
    pub anthropic_api_key: Option<CredentialGuard<String>>,
    /// `OpenRouter` API key.
    pub openrouter_api_key: Option<CredentialGuard<String>>,
    /// Cerebras API key.
    pub cerebras_api_key: Option<CredentialGuard<String>>,
    /// Exa search API key.
    pub exa_api_key: Option<CredentialGuard<String>>,
    /// Voyage AI API key.
    pub voyage_api_key: Option<CredentialGuard<String>>,
    /// Ollama base URL (not a secret, but part of the credential config).
    pub ollama_base_url: Option<String>,
    /// Have I Been Pwned API key (for breach exposure checks).
    pub hibp_api_key: Option<CredentialGuard<String>>,
    /// GitHub personal access token.
    pub github_token: Option<CredentialGuard<String>>,
    /// FEC (Federal Election Commission) API key.
    pub fec_api_key: Option<CredentialGuard<String>>,
    /// `OpenCorporates` API token (optional — free tier works without one).
    pub opencorporates_api_key: Option<CredentialGuard<String>>,
    /// UK Companies House API key (free registration at developer.company-information.service.gov.uk).
    pub uk_companies_house_api_key: Option<CredentialGuard<String>>,
    /// OpenSanctions API key for entity matching and PEP screening.
    pub opensanctions_api_key: Option<CredentialGuard<String>>,
    /// MarineTraffic API key for vessel AIS lookups (marinetraffic.com).
    pub marinetraffic_api_key: Option<CredentialGuard<String>>,
}

impl CredentialBundle {
    /// Returns `true` if at least one credential key is set (non-empty).
    #[must_use]
    pub fn has_any(&self) -> bool {
        let has = |opt: &Option<CredentialGuard<String>>| -> bool {
            opt.as_ref().is_some_and(|g| !g.expose().trim().is_empty())
        };
        has(&self.openai_api_key)
            || has(&self.anthropic_api_key)
            || has(&self.openrouter_api_key)
            || has(&self.cerebras_api_key)
            || has(&self.exa_api_key)
            || has(&self.voyage_api_key)
            || has(&self.hibp_api_key)
            || has(&self.github_token)
            || self
                .ollama_base_url
                .as_ref()
                .is_some_and(|u| !u.trim().is_empty())
    }

    /// Fill in empty fields from a lower-priority bundle.
    pub fn merge_missing(&mut self, other: &Self) {
        macro_rules! fill {
            ($field:ident) => {
                if self.$field.is_none() {
                    self.$field = other.$field.clone();
                }
            };
        }
        fill!(openai_api_key);
        fill!(anthropic_api_key);
        fill!(openrouter_api_key);
        fill!(cerebras_api_key);
        fill!(exa_api_key);
        fill!(voyage_api_key);
        fill!(ollama_base_url);
        fill!(hibp_api_key);
        fill!(github_token);
        fill!(fec_api_key);
        fill!(opencorporates_api_key);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn credential_guard_debug_is_redacted() {
        let guard = CredentialGuard::new("super-secret-key".to_string());
        let debug = format!("{guard:?}");
        assert_eq!(debug, "***REDACTED***");
        assert!(!debug.contains("super-secret"));
    }

    #[test]
    fn credential_guard_display_is_redacted() {
        let guard = CredentialGuard::new("super-secret-key".to_string());
        let display = format!("{guard}");
        assert_eq!(display, "***REDACTED***");
    }

    #[test]
    fn credential_guard_deref() {
        let guard = CredentialGuard::new("my-api-key".to_string());
        let inner: &String = &guard;
        assert_eq!(inner, "my-api-key");
    }

    #[test]
    fn credential_guard_expose() {
        let guard = CredentialGuard::new("secret".to_string());
        assert_eq!(guard.expose(), "secret");
    }

    #[test]
    fn credential_bundle_debug_hides_secrets() {
        let bundle = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("sk-ant-12345".to_string())),
            ..Default::default()
        };
        let debug = format!("{bundle:?}");
        assert!(!debug.contains("sk-ant-12345"));
        assert!(debug.contains("***REDACTED***"));
    }

    #[test]
    fn credential_bundle_roundtrip_serde() {
        let bundle = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("sk-ant-test".to_string())),
            ollama_base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&bundle).unwrap();
        // The serialized form should contain the actual key (for storage)
        assert!(json.contains("sk-ant-test"));

        let restored: CredentialBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(
            restored.anthropic_api_key.as_ref().unwrap().expose(),
            "sk-ant-test"
        );
        assert_eq!(
            restored.ollama_base_url.as_ref().unwrap(),
            "http://localhost:11434"
        );
    }

    #[test]
    fn has_any_returns_false_when_all_none() {
        let bundle = CredentialBundle::default();
        assert!(!bundle.has_any());
    }

    #[test]
    fn has_any_returns_false_when_all_empty_strings() {
        let bundle = CredentialBundle {
            openai_api_key: Some(CredentialGuard::new("  ".to_string())),
            ..Default::default()
        };
        assert!(!bundle.has_any());
    }

    #[test]
    fn has_any_returns_true_when_one_set() {
        let bundle = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("sk-test".to_string())),
            ..Default::default()
        };
        assert!(bundle.has_any());
    }

    #[test]
    fn merge_missing_fills_empty_fields() {
        let mut high = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("high-key".to_string())),
            ..Default::default()
        };

        let low = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("low-key".to_string())),
            openai_api_key: Some(CredentialGuard::new("low-openai".to_string())),
            ollama_base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        };

        high.merge_missing(&low);

        // High-priority key preserved
        assert_eq!(
            high.anthropic_api_key.as_ref().unwrap().expose(),
            "high-key"
        );
        // Low-priority key fills in missing
        assert_eq!(high.openai_api_key.as_ref().unwrap().expose(), "low-openai");
        assert_eq!(
            high.ollama_base_url.as_deref(),
            Some("http://localhost:11434")
        );
    }
}
