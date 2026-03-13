//! Credential bundle and `CredentialGuard<T>` newtype.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A credential value that masks its contents in Debug and Display output.
#[derive(Clone, Serialize, Deserialize)]
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

impl<T> fmt::Debug for CredentialGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialGuard(***)")
    }
}

impl<T> fmt::Display for CredentialGuard<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// Bundle of API keys and credentials resolved from multiple sources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialBundle {
    /// Anthropic API key.
    pub anthropic_api_key: Option<CredentialGuard<String>>,
    /// OpenAI API key.
    pub openai_api_key: Option<CredentialGuard<String>>,
    /// OpenRouter API key.
    pub openrouter_api_key: Option<CredentialGuard<String>>,
    /// Cerebras API key.
    pub cerebras_api_key: Option<CredentialGuard<String>>,
    /// Brave Search API key.
    pub brave_api_key: Option<CredentialGuard<String>>,
    /// Ollama base URL (not a secret, but part of the credential config).
    pub ollama_base_url: Option<String>,
    /// Custom OpenAI-compatible base URL.
    pub openai_base_url: Option<String>,
}
