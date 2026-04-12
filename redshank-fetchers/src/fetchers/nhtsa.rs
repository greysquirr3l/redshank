//! NHTSA — National Highway Traffic Safety Administration complaints and recalls.
//!
//! API: <https://api.nhtsa.gov/complaints/complaintsByVehicle>
//! API: <https://api.nhtsa.gov/recalls/recallsByVehicle>
//! No authentication required.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const COMPLAINTS_API: &str = "https://api.nhtsa.gov/complaints/complaintsByVehicle";
const RECALLS_API: &str = "https://api.nhtsa.gov/recalls/recallsByVehicle";

/// Fetch NHTSA complaints for a specific vehicle.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_complaints(
    make: &str,
    model: &str,
    year: u16,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    let resp = client
        .get(COMPLAINTS_API)
        .query(&[
            ("make", make),
            ("model", model),
            ("modelYear", &year.to_string()),
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
    let results = extract_results(&json);

    let output_path = output_dir.join("nhtsa_complaints.ndjson");
    let count = write_ndjson(&output_path, &results)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "nhtsa-complaints".into(),
        attribution: None,
    })
}

/// Fetch NHTSA recalls for a specific vehicle.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_recalls(
    make: &str,
    model: &str,
    year: u16,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    let resp = client
        .get(RECALLS_API)
        .query(&[
            ("make", make),
            ("model", model),
            ("modelYear", &year.to_string()),
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
    let results = extract_results(&json);

    let output_path = output_dir.join("nhtsa_recalls.ndjson");
    let count = write_ndjson(&output_path, &results)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "nhtsa-recalls".into(),
        attribution: None,
    })
}

/// Extract results array from NHTSA response.
fn extract_results(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("results")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Extracted complaint details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplaintDetails {
    /// Vehicle manufacturer.
    pub manufacturer: String,
    /// Vehicle component involved.
    pub component: String,
    /// Complaint description.
    pub description: String,
    /// Whether crash occurred.
    pub crash: bool,
    /// Whether injury occurred.
    pub injury: bool,
    /// Whether death occurred.
    pub death: bool,
}

/// Extract complaint details from a NHTSA complaint record.
#[must_use]
pub fn extract_complaint_details(record: &serde_json::Value) -> Option<ComplaintDetails> {
    Some(ComplaintDetails {
        manufacturer: record
            .get("manufacturer")
            .or_else(|| record.get("make"))
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        component: record
            .get("components")
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        description: record
            .get("summary")
            .or_else(|| record.get("description"))
            .and_then(serde_json::Value::as_str)?
            .to_string(),
        crash: record
            .get("crash")
            .and_then(serde_json::Value::as_bool)
            .or_else(|| {
                record
                    .get("crash")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "Y")
            })
            .unwrap_or(false),
        injury: record
            .get("injuries")
            .and_then(serde_json::Value::as_bool)
            .or_else(|| {
                record
                    .get("injuries")
                    .and_then(|v| v.as_str())
                    .map(|s| s != "0" && s != "N")
            })
            .unwrap_or(false),
        death: record
            .get("deaths")
            .and_then(serde_json::Value::as_bool)
            .or_else(|| {
                record
                    .get("deaths")
                    .and_then(|v| v.as_str())
                    .map(|s| s != "0" && s != "N")
            })
            .unwrap_or(false),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn nhtsa_parses_complaints_response() {
        let mock_json = serde_json::json!({
            "count": 2,
            "results": [
                {
                    "odiNumber": 12_345_678,
                    "manufacturer": "TOYOTA",
                    "make": "TOYOTA",
                    "model": "CAMRY",
                    "modelYear": 2020,
                    "components": "AIR BAGS",
                    "summary": "Air bag deployed without collision",
                    "crash": false,
                    "injuries": false,
                    "deaths": false
                },
                {
                    "odiNumber": 87_654_321,
                    "manufacturer": "TOYOTA",
                    "make": "TOYOTA",
                    "model": "CAMRY",
                    "modelYear": 2020,
                    "components": "BRAKES",
                    "summary": "Brake failure at highway speed",
                    "crash": true,
                    "injuries": true,
                    "deaths": false
                }
            ]
        });

        let results = extract_results(&mock_json);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["components"], "AIR BAGS");
        assert_eq!(results[1]["crash"], true);
    }

    #[test]
    fn nhtsa_extracts_complaint_details() {
        let record = serde_json::json!({
            "manufacturer": "FORD",
            "components": "FUEL SYSTEM",
            "summary": "Fuel leak detected near engine",
            "crash": false,
            "injuries": false,
            "deaths": false
        });

        let details = extract_complaint_details(&record).unwrap();
        assert_eq!(details.manufacturer, "FORD");
        assert_eq!(details.component, "FUEL SYSTEM");
        assert!(!details.crash);
        assert!(!details.injury);
        assert!(!details.death);
    }

    #[test]
    fn nhtsa_parses_recalls_response() {
        let mock_json = serde_json::json!({
            "count": 1,
            "results": [
                {
                    "nhtsaCampaignNumber": "24V123000",
                    "manufacturer": "HONDA",
                    "component": "ENGINE",
                    "summary": "Engine may stall unexpectedly"
                }
            ]
        });

        let results = extract_results(&mock_json);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["nhtsaCampaignNumber"], "24V123000");
    }
}
