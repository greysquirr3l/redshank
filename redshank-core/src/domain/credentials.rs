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
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Access the inner value. Only use where the credential is actually needed.
    pub fn expose(&self) -> &T {
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

/// Bundle of API keys and credentials resolved from multiple sources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialBundle {
    /// OpenAI API key.
    pub openai_api_key: Option<CredentialGuard<String>>,
    /// Anthropic API key.
    pub anthropic_api_key: Option<CredentialGuard<String>>,
    /// OpenRouter API key.
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
}

#[cfg(test)]
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
        let mut bundle = CredentialBundle::default();
        bundle.anthropic_api_key = Some(CredentialGuard::new("sk-ant-12345".to_string()));
        let debug = format!("{bundle:?}");
        assert!(!debug.contains("sk-ant-12345"));
        assert!(debug.contains("***REDACTED***"));
    }

    #[test]
    fn credential_bundle_roundtrip_serde() {
        let mut bundle = CredentialBundle::default();
        bundle.anthropic_api_key = Some(CredentialGuard::new("sk-ant-test".to_string()));
        bundle.ollama_base_url = Some("http://localhost:11434".to_string());

        let json = serde_json::to_string(&bundle).unwrap();
        // The serialized form should contain the actual key (for storage)
        assert!(json.contains("sk-ant-test"));

        let restored: CredentialBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.anthropic_api_key.as_ref().unwrap().expose(), "sk-ant-test");
        assert_eq!(restored.ollama_base_url.as_ref().unwrap(), "http://localhost:11434");
    }
}
