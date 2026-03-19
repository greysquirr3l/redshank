//! `FinCEN` BOI — Beneficial Ownership Information database.
//!
//! API: POST `https://boiefiling.fincen.gov/api/v1/search`
//! Requires API key in `Authorization: Bearer` header.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://boiefiling.fincen.gov/api/v1";

/// Search the `FinCEN` Beneficial Ownership database for entities matching `query`.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_boi_entities(
    query: &str,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let body = serde_json::json!({
            "query": query,
            "page": page,
        });

        let resp = client
            .post(format!("{API_BASE}/search"))
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(FetchError::ApiError {
                status: status.as_u16(),
                body: text,
            });
        }

        let json: serde_json::Value = resp.json().await?;
        let results = json
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        let total_pages = u32::try_from(
            json.get("pagination")
                .and_then(|p| p.get("totalPages"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1),
        )
        .unwrap_or(u32::MAX);

        if page >= total_pages {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("fincen_boi.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "fincen_boi".into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    #[test]
    fn fincen_boi_parses_entity_response() {
        let mock = serde_json::json!({
            "results": [
                {
                    "entityName": "ACME SHELL CORP LLC",
                    "jurisdiction": "DE",
                    "beneficialOwners": [
                        {"name": "John Doe", "dobHash": "a1b2c3", "address": "123 Main St, Wilmington, DE"}
                    ]
                }
            ],
            "pagination": {"page": 1, "totalPages": 1}
        });
        let results = mock.get("results").and_then(|v| v.as_array()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["entityName"], "ACME SHELL CORP LLC");
        let owners = results[0]["beneficialOwners"].as_array().unwrap();
        assert_eq!(owners[0]["name"], "John Doe");
    }
}
