//! NLRB — National Labor Relations Board case data.
//!
//! API: <https://www.nlrb.gov/reports/open-reports> (bulk data)
//! Note: The main case search at <https://www.nlrb.gov/search/case> requires
//! JavaScript rendering via stygian-browser.
//!
//! This implementation uses the RSS/JSON feeds from open reports.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://www.nlrb.gov/api/cases";
const DEFAULT_PER_PAGE: u32 = 50;

/// Fetch NLRB case data for a given employer or union query.
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
                ("search", query),
                ("page", &page.to_string()),
                ("per_page", &DEFAULT_PER_PAGE.to_string()),
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

    let output_path = output_dir.join("nlrb_cases.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "nlrb".into(),
        attribution: None,
    })
}

/// Extract case records from the NLRB response.
fn extract_cases(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("cases")
        .or_else(|| json.get("results"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Extracted NLRB case details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseDetails {
    /// NLRB case number.
    pub case_number: String,
    /// Name of the employer involved.
    pub employer_name: String,
    /// Union name (if applicable).
    pub union_name: Option<String>,
    /// Type of charge (ULP, representation, etc.).
    pub charge_type: String,
    /// NLRB region handling the case.
    pub region: String,
    /// Current case status.
    pub status: Option<String>,
}

/// Extract NLRB case details from a record.
#[must_use]
pub fn extract_case_details(record: &serde_json::Value) -> Option<CaseDetails> {
    Some(CaseDetails {
        case_number: record
            .get("case_number")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        employer_name: record
            .get("employer_name")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        union_name: record
            .get("union_name")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        charge_type: record
            .get("charge_type")
            .or_else(|| record.get("case_type"))
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        region: record
            .get("region")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        status: record
            .get("status")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn nlrb_parses_case_response() {
        let mock_json = serde_json::json!({
            "cases": [
                {
                    "case_number": "01-CA-123456",
                    "employer_name": "ACME CORP",
                    "union_name": "Local 123",
                    "charge_type": "ULP",
                    "region": "01",
                    "status": "Open"
                },
                {
                    "case_number": "02-RC-789012",
                    "employer_name": "WIDGETS INC",
                    "charge_type": "RC",
                    "region": "02",
                    "status": "Closed"
                }
            ]
        });

        let cases = extract_cases(&mock_json);
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0]["case_number"], "01-CA-123456");
        assert_eq!(cases[1]["charge_type"], "RC");
    }

    #[test]
    fn nlrb_extracts_case_details() {
        let record = serde_json::json!({
            "case_number": "01-CA-999999",
            "employer_name": "BIG COMPANY",
            "union_name": "Workers United",
            "charge_type": "ULP",
            "region": "01",
            "status": "Pending"
        });

        let details = extract_case_details(&record).unwrap();
        assert_eq!(details.case_number, "01-CA-999999");
        assert_eq!(details.employer_name, "BIG COMPANY");
        assert_eq!(details.union_name, Some("Workers United".to_string()));
        assert_eq!(details.charge_type, "ULP");
    }

    #[test]
    fn nlrb_handles_missing_union() {
        let record = serde_json::json!({
            "case_number": "02-RC-111111",
            "employer_name": "SOLO CORP",
            "charge_type": "RC",
            "region": "02"
        });

        let details = extract_case_details(&record).unwrap();
        assert!(details.union_name.is_none());
        assert!(details.status.is_none());
    }
}
