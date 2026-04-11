//! CFPB — Consumer Financial Protection Bureau complaint database.
//!
//! API: <https://www.consumerfinance.gov/data-research/consumer-complaints/search/api/v1/>
//! Pagination: offset-based (size + from), max size 100.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str =
    "https://www.consumerfinance.gov/data-research/consumer-complaints/search/api/v1/";
const DEFAULT_SIZE: u32 = 100;

/// Fetch CFPB consumer complaints for a given company search term.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_complaints(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let from = page * DEFAULT_SIZE;
        let resp = client
            .get(API_BASE)
            .query(&[
                ("search_term", query),
                ("size", &DEFAULT_SIZE.to_string()),
                ("from", &from.to_string()),
                ("sort", "created_date_desc"),
            ])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(FetchError::ApiError {
                status: status.as_u16(),
                body,
            });
        }

        let json: serde_json::Value = resp.json().await?;
        let hits = extract_hits(&json);

        if hits.is_empty() {
            break;
        }
        all_records.extend(hits);

        // Check if we've fetched all results
        let total = json
            .get("hits")
            .and_then(|h| h.get("total"))
            .and_then(|t| t.get("value"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if u64::from(from + DEFAULT_SIZE) >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("cfpb_complaints.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "cfpb".into(),
        attribution: None,
    })
}

/// Extract hits from the CFPB response, flattening the nested `_source` structure.
fn extract_hits(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("hits")
        .and_then(|h| h.get("hits"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|hit| hit.get("_source").cloned())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn cfpb_parses_complaint_response() {
        let mock_json = serde_json::json!({
            "hits": {
                "total": {"value": 2},
                "hits": [
                    {
                        "_source": {
                            "complaint_id": "123456",
                            "product": "Credit card",
                            "issue": "Closing/Cancelling account",
                            "company": "ACME BANK",
                            "company_response": "Closed with explanation",
                            "state": "CA",
                            "complaint_what_happened": "They closed my card without notice."
                        }
                    },
                    {
                        "_source": {
                            "complaint_id": "789012",
                            "product": "Mortgage",
                            "issue": "Struggling to pay mortgage",
                            "company": "ACME BANK",
                            "company_response": "Closed with monetary relief",
                            "state": "NY"
                        }
                    }
                ]
            }
        });

        let hits = extract_hits(&mock_json);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0]["company"], "ACME BANK");
        assert_eq!(hits[0]["product"], "Credit card");
        assert_eq!(hits[1]["issue"], "Struggling to pay mortgage");
    }

    #[test]
    fn cfpb_handles_empty_response() {
        let empty = serde_json::json!({
            "hits": {
                "total": {"value": 0},
                "hits": []
            }
        });
        let hits = extract_hits(&empty);
        assert!(hits.is_empty());
    }
}
