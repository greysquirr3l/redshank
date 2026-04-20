//! `HaveIBeenPwnd` breach metadata lookup.
//!
//! This fetcher is an explicit source-ID wrapper around the existing HIBP
//! integration, using the same paid API key and returning breach metadata only.

use crate::domain::{FetchError, FetchOutput};
use crate::fetchers::hibp::{HibpApiKey, fetch_breaches_for_email};
use std::path::Path;

/// Fetch breach metadata for an email using the Have I Been Pwned API.
///
/// # Errors
///
/// Returns `Err` when `email` or `api_key` is empty, or when the upstream API
/// request/parsing fails.
pub async fn fetch_haveibeenpwnd(
    email: &str,
    output_dir: &Path,
    api_key: &str,
) -> Result<FetchOutput, FetchError> {
    if email.trim().is_empty() {
        return Err(FetchError::Parse(
            "email query must not be empty".to_string(),
        ));
    }

    if api_key.trim().is_empty() {
        return Err(FetchError::Other(
            "missing required credential: HIBP_API_KEY".to_string(),
        ));
    }

    let key = HibpApiKey::new(api_key.to_string());
    let mut output = fetch_breaches_for_email(email, &key, output_dir).await?;
    output.source_name = "haveibeenpwnd".to_string();
    Ok(output)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_email_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let result = fetch_haveibeenpwnd("", tmp.path(), "test-key").await;
        match result {
            Err(FetchError::Parse(message)) => assert!(message.contains("must not be empty")),
            other => panic!("expected FetchError::Parse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_api_key_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let result = fetch_haveibeenpwnd("user@example.com", tmp.path(), "").await;
        match result {
            Err(FetchError::Other(message)) => {
                assert!(message.contains("missing required credential"));
            }
            other => panic!("expected FetchError::Other, got {other:?}"),
        }
    }
}
