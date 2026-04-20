//! Reverse phone lookup via `TrueCaller` API.
//!
//! Provides full subscriber information (name, address, carrier) for phone numbers.
//! Requires `TRUECALLER_API_KEY` credential.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const TRUECALLER_API: &str = "https://api.truecaller.com/v1/search";

/// Fetch subscriber information for a phone number via `TrueCaller`.
///
/// # Errors
///
/// Returns `Err` if credentials are unavailable, the request fails,
/// the API returns a non-success status, or output cannot be written.
pub async fn fetch_reverse_phone_truecaller(
    phone: &str,
    output_dir: &Path,
    api_key: &str,
) -> Result<FetchOutput, FetchError> {
    let normalized = phone.trim();
    if normalized.is_empty() {
        return Err(FetchError::Parse(
            "phone number must not be empty for reverse_phone_truecaller".to_string(),
        ));
    }

    if api_key.is_empty() {
        return Err(FetchError::Other(
            "TRUECALLER_API_KEY is required".to_string(),
        ));
    }

    let client = build_client()?;
    let response = client
        .get(TRUECALLER_API)
        .query(&[("phone", normalized), ("countryCode", "US")])
        .header("Authorization", format!("Bearer {api_key}"))
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
        "source": "truecaller",
        "phone_number": payload.get("phoneNumber").and_then(serde_json::Value::as_str),
        "name": payload.get("name").and_then(serde_json::Value::as_str),
        "email": payload.get("email").and_then(serde_json::Value::as_str),
        "address": payload.get("address").and_then(serde_json::Value::as_str),
        "city": payload.get("city").and_then(serde_json::Value::as_str),
        "state": payload.get("state").and_then(serde_json::Value::as_str),
        "zip_code": payload.get("zipCode").and_then(serde_json::Value::as_str),
        "carrier": payload.get("carrier").and_then(serde_json::Value::as_str),
        "line_type": payload.get("lineType").and_then(serde_json::Value::as_str),
        "first_seen": payload.get("firstSeen").and_then(serde_json::Value::as_str),
    });

    let output_path = output_dir.join("reverse_phone_truecaller.ndjson");
    let records_written = write_ndjson(&output_path, &[record])?;

    Ok(FetchOutput {
        records_written,
        output_path,
        source_name: "reverse_phone_truecaller".to_string(),
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
        let err = fetch_reverse_phone_truecaller("", dir.path(), "test_key")
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Parse(_)));
    }

    #[tokio::test]
    async fn missing_credentials_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = fetch_reverse_phone_truecaller("+14155552671", dir.path(), "")
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Other(_)));
    }
}
