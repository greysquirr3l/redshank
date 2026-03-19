//! `USASpending` — Federal spending data.
//!
//! API: POST <https://api.usaspending.gov/api/v2/search/spending_by_award/>
//! Pagination: page-based (1-indexed), limit max 500.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.usaspending.gov/api/v2";

/// Fetch `USASpending` award data for the given recipient query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_awards(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let body = serde_json::json!({
            "filters": {
                "recipient_search_text": [query],
                "award_type_codes": ["A", "B", "C", "D"]
            },
            "page": page,
            "limit": 100,
            "sort": "Award Amount",
            "order": "desc"
        });

        let resp = client
            .post(format!("{API_BASE}/search/spending_by_award/"))
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

        let has_next = json
            .get("page_metadata")
            .and_then(|p| p.get("hasNext"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if !has_next {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("usaspending.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "usaspending".into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    #[test]
    fn usaspending_parses_award_response() {
        let mock = serde_json::json!({
            "results": [
                {
                    "Award ID": "CONT_AWD_001",
                    "Recipient Name": "ACME CORP",
                    "Award Amount": 1_000_000.0,
                    "Awarding Agency": "Department of Defense"
                }
            ],
            "page_metadata": {"page": 1, "hasNext": false, "total": 1}
        });
        let results = mock["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["Recipient Name"], "ACME CORP");
    }
}
