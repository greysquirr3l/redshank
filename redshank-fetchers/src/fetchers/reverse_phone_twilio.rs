//! Reverse phone lookup via Twilio Lookup API.
//!
//! Provides carrier detection, line type, and basic validation via Twilio.
//! Requires `TWILIO_ACCOUNT_SID` and `TWILIO_AUTH_TOKEN` credentials.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const TWILIO_LOOKUP_API: &str = "https://lookups.twilio.com/v1/PhoneNumbers";

/// Fetch carrier and line-type info for a phone number via Twilio Lookup.
///
/// # Errors
///
/// Returns `Err` if credentials are unavailable, the request fails,
/// the API returns a non-success status, or output cannot be written.
pub async fn fetch_reverse_phone_twilio(
    phone: &str,
    output_dir: &Path,
    account_sid: &str,
    auth_token: &str,
) -> Result<FetchOutput, FetchError> {
    let normalized = phone.trim();
    if normalized.is_empty() {
        return Err(FetchError::Parse(
            "phone number must not be empty for reverse_phone_twilio".to_string(),
        ));
    }

    if account_sid.is_empty() || auth_token.is_empty() {
        return Err(FetchError::Other(
            "TWILIO_ACCOUNT_SID and TWILIO_AUTH_TOKEN are required".to_string(),
        ));
    }

    let client = build_client()?;
    let response = client
        .get(format!("{TWILIO_LOOKUP_API}/{normalized}"))
        .query(&[("Type", "carrier")])
        .basic_auth(account_sid, Some(auth_token))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let payload: serde_json::Value = response.json().await?;
    let record = serde_json::json!({
        "query": normalized,
        "source": "twilio",
        "phone_number": payload.get("phone_number").and_then(serde_json::Value::as_str),
        "country_code": payload.get("country_code").and_then(serde_json::Value::as_str),
        "carrier": {
            "name": payload.get("carrier").and_then(|c| c.get("name")).and_then(serde_json::Value::as_str),
            "type": payload.get("carrier").and_then(|c| c.get("type")).and_then(serde_json::Value::as_str),
        },
        "friendly_name": payload.get("friendly_name").and_then(serde_json::Value::as_str),
    });

    let output_path = output_dir.join("reverse_phone_twilio.ndjson");
    let records_written = write_ndjson(&output_path, &[record])?;

    Ok(FetchOutput {
        records_written,
        output_path,
        source_name: "reverse_phone_twilio".to_string(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_phone_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = fetch_reverse_phone_twilio("", dir.path(), "test_sid", "test_token")
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Parse(_)));
    }

    #[tokio::test]
    async fn missing_credentials_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = fetch_reverse_phone_twilio("+14155552671", dir.path(), "", "token")
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Other(_)));
    }
}
