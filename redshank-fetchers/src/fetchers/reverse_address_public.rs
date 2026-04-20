//! Reverse address lookup using public geocoding endpoints.
//!
//! Uses the free U.S. Census geocoder to normalize and enrich address strings.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const CENSUS_GEOCODER_API: &str =
    "https://geocoding.geo.census.gov/geocoder/locations/onelineaddress";

/// Fetch normalized/geocoded address matches from public Census geocoder data.
///
/// # Errors
///
/// Returns `Err` if the request fails, the API returns a non-success status,
/// or output cannot be written.
pub async fn fetch_reverse_address_public(
    address: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let normalized = address.trim();
    if normalized.is_empty() {
        return Err(FetchError::Parse(
            "address must not be empty for reverse_address_public".to_string(),
        ));
    }

    let client = build_client()?;
    let response = client
        .get(CENSUS_GEOCODER_API)
        .query(&[
            ("address", normalized),
            ("benchmark", "Public_AR_Current"),
            ("vintage", "Current_Current"),
            ("format", "json"),
        ])
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
    let matches = payload
        .get("result")
        .and_then(|r| r.get("addressMatches"))
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let mut records = matches
        .iter()
        .map(|m| {
            serde_json::json!({
                "query": normalized,
                "source": "us_census_geocoder",
                "matched": true,
                "matched_address": m.get("matchedAddress").and_then(serde_json::Value::as_str),
                "coordinates": m.get("coordinates"),
                "tiger_line": m.get("tigerLine"),
                "address_components": m.get("addressComponents"),
            })
        })
        .collect::<Vec<_>>();

    if records.is_empty() {
        records.push(serde_json::json!({
            "query": normalized,
            "source": "us_census_geocoder",
            "matched": false,
            "notes": ["No Census geocoder match found for provided address"],
        }));
    }

    let output_path = output_dir.join("reverse_address_public.ndjson");
    let records_written = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written,
        output_path,
        source_name: "reverse_address_public".to_string(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_address_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = fetch_reverse_address_public("", dir.path())
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Parse(_)));
    }
}
