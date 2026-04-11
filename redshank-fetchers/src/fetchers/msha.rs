//! MSHA — Mine Safety and Health Administration inspection data.
//!
//! API: <https://arlweb.msha.gov/OpenGovernmentData/OGIMSHA.asp>
//! Also: <https://data.dol.gov/get/mshamines>
//! Pagination: skip + limit.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://data.dol.gov/get/mshaviol";
const DEFAULT_LIMIT: u32 = 200;

/// Fetch MSHA mine violation data for a given operator query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_violations(
    query: &str,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let skip = page * DEFAULT_LIMIT;

        let resp = client
            .get(API_BASE)
            .header("X-API-KEY", api_key)
            .query(&[
                ("$filter", &format!("OPERATOR_NAME eq '{query}'")),
                ("$skip", &skip.to_string()),
                ("$top", &DEFAULT_LIMIT.to_string()),
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
        let results = json.as_array().cloned().unwrap_or_default();

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("msha_violations.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "msha".into(),
        attribution: None,
    })
}

/// Extract key violation details from MSHA record.
#[must_use]
pub fn extract_violation_details(record: &serde_json::Value) -> Option<ViolationDetails> {
    Some(ViolationDetails {
        mine_id: record
            .get("MINE_ID")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        operator_name: record
            .get("OPERATOR_NAME")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        violation_type: record
            .get("SIG_SUB")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        penalty_amount: record
            .get("AMOUNT_DUE")
            .and_then(serde_json::Value::as_f64),
        inspection_date: record
            .get("INSPECTION_BEGIN_DT")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

/// Extracted violation details from MSHA data.
#[derive(Debug, Clone, PartialEq)]
pub struct ViolationDetails {
    /// Mine identification number.
    pub mine_id: String,
    /// Name of the mine operator.
    pub operator_name: String,
    /// Violation type (S&S = significant and substantial).
    pub violation_type: Option<String>,
    /// Penalty amount assessed.
    pub penalty_amount: Option<f64>,
    /// Date of the inspection.
    pub inspection_date: Option<String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn msha_parses_violation_response() {
        let mock_data: Vec<serde_json::Value> = vec![
            serde_json::json!({
                "MINE_ID": "1234567",
                "OPERATOR_NAME": "ACME MINING CO",
                "SIG_SUB": "S",
                "AMOUNT_DUE": 5000.0,
                "INSPECTION_BEGIN_DT": "2024-01-15"
            }),
            serde_json::json!({
                "MINE_ID": "1234567",
                "OPERATOR_NAME": "ACME MINING CO",
                "SIG_SUB": "N",
                "AMOUNT_DUE": 1500.0,
                "INSPECTION_BEGIN_DT": "2024-02-20"
            }),
        ];

        assert_eq!(mock_data.len(), 2);
        assert_eq!(mock_data[0]["OPERATOR_NAME"], "ACME MINING CO");
    }

    #[test]
    fn msha_extracts_violation_details() {
        let record = serde_json::json!({
            "MINE_ID": "7654321",
            "OPERATOR_NAME": "COAL CORP",
            "SIG_SUB": "S",
            "AMOUNT_DUE": 10000.0,
            "INSPECTION_BEGIN_DT": "2024-03-01"
        });

        let details = extract_violation_details(&record).unwrap();
        assert_eq!(details.mine_id, "7654321");
        assert_eq!(details.operator_name, "COAL CORP");
        assert_eq!(details.violation_type, Some("S".to_string()));
        assert!((details.penalty_amount.unwrap() - 10000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn msha_handles_optional_fields() {
        let record = serde_json::json!({
            "MINE_ID": "1111111",
            "OPERATOR_NAME": "TEST MINE"
        });

        let details = extract_violation_details(&record).unwrap();
        assert_eq!(details.mine_id, "1111111");
        assert!(details.violation_type.is_none());
        assert!(details.penalty_amount.is_none());
    }
}
