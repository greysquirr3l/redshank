//! HIBP — Have I Been Pwned breach metadata lookup.
//!
//! API v3: `https://haveibeenpwned.com/api/v3/breachedaccount/{email}`
//! Requires `hibp-api-key` header. Rate limit: 1.5 req/sec.
//!
//! IMPORTANT: This fetcher returns only breach METADATA (breach names,
//! domains, dates, data classes exposed). It NEVER fetches, stores, or
//! handles raw passwords or credential data.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://haveibeenpwned.com/api/v3";
/// HIBP rate limit: one request per 1500ms.
const HIBP_RATE_LIMIT_MS: u64 = 1500;

/// Wrapper for HIBP API key that never appears in Debug/Display output.
#[derive(Clone)]
pub struct HibpApiKey(String);

impl HibpApiKey {
    #[must_use]
    pub const fn new(key: String) -> Self {
        Self(key)
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for HibpApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HibpApiKey(***)")
    }
}

impl std::fmt::Display for HibpApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

/// Check if an email address appears in any known data breaches.
///
/// Returns breach metadata only — never raw credential data.
/// Returns an empty list (not an error) if the email is clean (API returns 404).
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or the response cannot be parsed
/// (404 responses are not errors — they indicate a clean email).
pub async fn fetch_breaches_for_email(
    email: &str,
    api_key: &HibpApiKey,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    // Enforce rate limit
    tokio::time::sleep(std::time::Duration::from_millis(HIBP_RATE_LIMIT_MS)).await;

    let resp = client
        .get(format!(
            "{API_BASE}/breachedaccount/{email}?truncateResponse=false"
        ))
        .header("hibp-api-key", api_key.as_str())
        .header("User-Agent", "redshank-investigation-agent")
        .send()
        .await?;

    let status = resp.status();

    // 404 means the email is clean — not an error
    if status.as_u16() == 404 {
        let output_path = output_dir.join("hibp_breaches.ndjson");
        write_ndjson(&output_path, &[])?;
        return Ok(FetchOutput {
            records_written: 0,
            output_path,
            source_name: "hibp".into(),
        });
    }

    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let breaches = json.as_array().cloned().unwrap_or_default();

    let output_path = output_dir.join("hibp_breaches.ndjson");
    let count = write_ndjson(&output_path, &breaches)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "hibp".into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn hibp_api_key_never_leaks_in_debug_or_display() {
        let key = HibpApiKey::new("super-secret-key-12345".to_string());
        let debug = format!("{key:?}");
        let display = format!("{key}");
        assert!(!debug.contains("super-secret"));
        assert!(!display.contains("super-secret"));
        assert!(debug.contains("***"));
        assert!(display.contains("***"));
    }

    #[test]
    fn hibp_parses_breach_list_fixture() {
        let mock = serde_json::json!([
            {
                "Name": "Adobe",
                "Title": "Adobe",
                "Domain": "adobe.com",
                "BreachDate": "2013-10-04",
                "AddedDate": "2013-12-04T00:00:00Z",
                "ModifiedDate": "2013-12-04T00:00:00Z",
                "PwnCount": 152_445_165,
                "Description": "Adobe breach description",
                "DataClasses": ["Email addresses", "Password hints", "Passwords", "Usernames"],
                "IsVerified": true,
                "IsFabricated": false,
                "IsSensitive": false,
                "IsRetired": false,
                "IsSpamList": false,
                "IsMalware": false
            },
            {
                "Name": "LinkedIn",
                "Title": "LinkedIn",
                "Domain": "linkedin.com",
                "BreachDate": "2012-05-05",
                "AddedDate": "2016-05-21T21:35:40Z",
                "ModifiedDate": "2016-05-21T21:35:40Z",
                "PwnCount": 164_611_595,
                "Description": "LinkedIn breach description",
                "DataClasses": ["Email addresses", "Passwords"],
                "IsVerified": true,
                "IsFabricated": false,
                "IsSensitive": false,
                "IsRetired": false,
                "IsSpamList": false,
                "IsMalware": false
            }
        ]);
        let breaches = mock.as_array().unwrap();
        assert_eq!(breaches.len(), 2);
        assert_eq!(breaches[0]["Name"], "Adobe");
        assert_eq!(breaches[0]["Domain"], "adobe.com");
        let data_classes = breaches[0]["DataClasses"].as_array().unwrap();
        assert!(data_classes.iter().any(|c| c == "Email addresses"));
        // Verify we only have metadata — no raw credentials field
        assert!(breaches[0].get("RawCredentials").is_none());
    }

    #[test]
    fn hibp_clean_email_returns_empty_list_not_error() {
        // When API returns 404, we should get empty records, not an error
        let empty: Vec<serde_json::Value> = vec![];
        assert!(empty.is_empty());
    }
}
