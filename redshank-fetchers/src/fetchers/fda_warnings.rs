//! FDA — Food and Drug Administration warning letters and enforcement reports.
//!
//! API: <https://api.fda.gov/drug/enforcement.json>
//! Pagination: skip + limit.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.fda.gov/drug/enforcement.json";
const DEFAULT_LIMIT: u32 = 100;

/// Fetch FDA enforcement reports for a given company/product search term.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_enforcement(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let skip = page * DEFAULT_LIMIT;
        let search_query = format!("recalling_firm:\"{query}\"");

        let resp = client
            .get(API_BASE)
            .query(&[
                ("search", search_query.as_str()),
                ("limit", &DEFAULT_LIMIT.to_string()),
                ("skip", &skip.to_string()),
            ])
            .send()
            .await?;

        let status = resp.status();

        // FDA API returns 404 when no results found
        if status.as_u16() == 404 {
            break;
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(FetchError::ApiError {
                status: status.as_u16(),
                body,
            });
        }

        let json: serde_json::Value = resp.json().await?;
        let results = extract_results(&json);

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        // Check pagination meta
        let total = json
            .get("meta")
            .and_then(|m| m.get("results"))
            .and_then(|r| r.get("total"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if u64::from(skip + DEFAULT_LIMIT) >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("fda_enforcement.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "fda-warnings".into(),
        attribution: None,
    })
}

/// Extract results from FDA enforcement response.
fn extract_results(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("results")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Extract key enforcement details from a record.
#[must_use]
pub fn extract_enforcement_details(
    record: &serde_json::Value,
) -> Option<(String, String, String)> {
    let company = record
        .get("recalling_firm")
        .and_then(serde_json::Value::as_str)?;
    let date = record
        .get("report_date")
        .and_then(serde_json::Value::as_str)?;
    let reason = record
        .get("reason_for_recall")
        .and_then(serde_json::Value::as_str)?;
    Some((company.to_string(), date.to_string(), reason.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn fda_parses_enforcement_response() {
        let mock_json = serde_json::json!({
            "meta": {
                "results": {"total": 2, "skip": 0, "limit": 100}
            },
            "results": [
                {
                    "recall_number": "D-1234-2024",
                    "recalling_firm": "ACME PHARMA INC",
                    "report_date": "20240115",
                    "reason_for_recall": "cGMP deviation: contamination detected",
                    "classification": "Class II",
                    "product_type": "Drug"
                },
                {
                    "recall_number": "D-5678-2024",
                    "recalling_firm": "ACME PHARMA INC",
                    "report_date": "20240220",
                    "reason_for_recall": "Labeling: incorrect dosage instructions",
                    "classification": "Class III",
                    "product_type": "Drug"
                }
            ]
        });

        let results = extract_results(&mock_json);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["recalling_firm"], "ACME PHARMA INC");
        assert_eq!(results[1]["classification"], "Class III");
    }

    #[test]
    fn fda_extracts_enforcement_details() {
        let record = serde_json::json!({
            "recalling_firm": "PHARMA CORP",
            "report_date": "20240315",
            "reason_for_recall": "Adulteration detected"
        });

        let details = extract_enforcement_details(&record).unwrap();
        assert_eq!(details.0, "PHARMA CORP");
        assert_eq!(details.1, "20240315");
        assert_eq!(details.2, "Adulteration detected");
    }

    #[test]
    fn fda_handles_empty_response() {
        let empty = serde_json::json!({
            "meta": {"results": {"total": 0}},
            "results": []
        });
        let results = extract_results(&empty);
        assert!(results.is_empty());
    }
}
