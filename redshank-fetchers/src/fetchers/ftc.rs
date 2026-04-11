//! FTC — Federal Trade Commission enforcement actions.
//!
//! API: <https://www.ftc.gov/api/v1/case>
//! Pagination: page-based.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://www.ftc.gov/api/v1/case";
const DEFAULT_PER_PAGE: u32 = 50;

/// Fetch FTC enforcement cases matching the search query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_cases(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let resp = client
            .get(API_BASE)
            .query(&[
                ("searchterm", query),
                ("page", &page.to_string()),
                ("items_per_page", &DEFAULT_PER_PAGE.to_string()),
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
        let results = extract_cases(&json);

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("ftc_cases.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "ftc".into(),
        attribution: None,
    })
}

/// Extract case data from FTC response, normalizing the structure.
fn extract_cases(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("results")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Extract respondent names from a case record.
#[must_use]
pub fn extract_respondents(case: &serde_json::Value) -> Vec<String> {
    case.get("respondents")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.get("name").and_then(serde_json::Value::as_str))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn ftc_parses_case_response_and_extracts_respondents() {
        let mock_json = serde_json::json!({
            "results": [
                {
                    "case_id": "C-4567",
                    "case_name": "In the Matter of ACME Corp",
                    "case_type": "advertising",
                    "disposition": "consent decree",
                    "respondents": [
                        {"name": "ACME Corp"},
                        {"name": "John Doe"}
                    ]
                },
                {
                    "case_id": "C-4568",
                    "case_name": "FTC v. Fraud Inc",
                    "case_type": "privacy",
                    "disposition": "litigation"
                }
            ]
        });

        let cases = extract_cases(&mock_json);
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0]["case_type"], "advertising");
        assert_eq!(cases[1]["disposition"], "litigation");

        let respondents = extract_respondents(&cases[0]);
        assert_eq!(respondents.len(), 2);
        assert!(respondents.contains(&"ACME Corp".to_string()));
        assert!(respondents.contains(&"John Doe".to_string()));
    }

    #[test]
    fn ftc_handles_missing_respondents() {
        let case = serde_json::json!({
            "case_id": "C-1234",
            "case_name": "Test Case"
        });
        let respondents = extract_respondents(&case);
        assert!(respondents.is_empty());
    }
}
