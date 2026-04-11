//! CFTC — Commodity Futures Trading Commission enforcement actions.
//!
//! Data source: <https://www.cftc.gov/LawRegulation/Enforcement/EnforcementActions/index.htm>
//! Note: This endpoint requires scraping. Consider using RSS feeds or bulk data exports
//! when available.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://www.cftc.gov/api/enforcement";
const DEFAULT_PER_PAGE: u32 = 50;

/// Fetch CFTC enforcement actions.
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
        let results = extract_actions(&json);

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("cftc_enforcement.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "cftc".into(),
        attribution: None,
    })
}

/// Extract enforcement actions from CFTC response.
fn extract_actions(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("actions")
        .or_else(|| json.get("results"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Extracted CFTC enforcement action details.
#[derive(Debug, Clone, PartialEq)]
pub struct EnforcementDetails {
    /// Respondent name (individual or firm).
    pub respondent: String,
    /// Type of violation (fraud, manipulation, registration, etc.).
    pub violation_type: String,
    /// Civil monetary penalty amount.
    pub penalty: Option<f64>,
    /// Whether a trading ban was imposed.
    pub trading_ban: bool,
    /// Date of the enforcement action.
    pub date: Option<String>,
}

/// Extract enforcement details from a CFTC action record.
#[must_use]
pub fn extract_enforcement_details(record: &serde_json::Value) -> Option<EnforcementDetails> {
    Some(EnforcementDetails {
        respondent: record
            .get("respondent")
            .or_else(|| record.get("defendant"))
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        violation_type: record
            .get("violation_type")
            .or_else(|| record.get("charges"))
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        penalty: record
            .get("civil_monetary_penalty")
            .or_else(|| record.get("penalty"))
            .and_then(serde_json::Value::as_f64),
        trading_ban: record
            .get("trading_ban")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        date: record
            .get("date")
            .or_else(|| record.get("action_date"))
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn cftc_parses_enforcement_response() {
        let mock_json = serde_json::json!({
            "actions": [
                {
                    "respondent": "FRAUDULENT TRADING LLC",
                    "violation_type": "fraud",
                    "civil_monetary_penalty": 1_000_000.0,
                    "trading_ban": true,
                    "date": "2024-01-15"
                },
                {
                    "respondent": "JOHN DOE",
                    "violation_type": "manipulation",
                    "civil_monetary_penalty": 500_000.0,
                    "trading_ban": false,
                    "date": "2024-02-20"
                }
            ]
        });

        let actions = extract_actions(&mock_json);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0]["respondent"], "FRAUDULENT TRADING LLC");
        assert_eq!(actions[1]["violation_type"], "manipulation");
    }

    #[test]
    fn cftc_extracts_enforcement_details() {
        let record = serde_json::json!({
            "respondent": "CRYPTO SCAM INC",
            "violation_type": "fraud",
            "civil_monetary_penalty": 2_500_000.0,
            "trading_ban": true,
            "date": "2024-03-01"
        });

        let details = extract_enforcement_details(&record).unwrap();
        assert_eq!(details.respondent, "CRYPTO SCAM INC");
        assert_eq!(details.violation_type, "fraud");
        assert!((details.penalty.unwrap() - 2_500_000.0).abs() < f64::EPSILON);
        assert!(details.trading_ban);
        assert_eq!(details.date, Some("2024-03-01".to_string()));
    }

    #[test]
    fn cftc_handles_alternate_field_names() {
        let record = serde_json::json!({
            "defendant": "JANE DOE",
            "charges": "registration violation",
            "penalty": 100_000.0,
            "action_date": "2024-04-15"
        });

        let details = extract_enforcement_details(&record).unwrap();
        assert_eq!(details.respondent, "JANE DOE");
        assert_eq!(details.violation_type, "registration violation");
        assert!(!details.trading_ban);
    }

    #[test]
    fn cftc_handles_missing_optional_fields() {
        let record = serde_json::json!({
            "respondent": "MINIMAL RECORD",
            "violation_type": "unknown"
        });

        let details = extract_enforcement_details(&record).unwrap();
        assert!(details.penalty.is_none());
        assert!(details.date.is_none());
    }
}
