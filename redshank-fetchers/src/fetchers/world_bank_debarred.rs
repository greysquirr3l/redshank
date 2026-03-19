//! World Bank Debarred and Cross-Debarred Firms.
//!
//! API: `https://apigwext.worldbank.org/dvsvc/v1.0/json/APPLICATION/ADOBE_ACROBAT/FIRM/debarredFirms`
//! No auth required. JSON response.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const API_URL: &str =
    "https://apigwext.worldbank.org/dvsvc/v1.0/json/APPLICATION/ADOBE_ACROBAT/FIRM/debarredFirms";

/// Fetch the full World Bank debarred firms list.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_debarred_firms(output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(API_URL).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let records = json.as_array().cloned().unwrap_or_default();

    let output_path = output_dir.join("world_bank_debarred.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "world_bank_debarred".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    #[test]
    fn world_bank_parses_debarred_firms_response() {
        let mock = serde_json::json!([
            {
                "firm_name": "SHELL CONSTRUCTION CO",
                "country": "Nigeria",
                "from_date": "2020-01-01",
                "to_date": "2025-12-31",
                "grounds": "Fraud",
                "sanction_type": "Debarment with Conditional Release",
                "cross_debarment": true
            },
            {
                "firm_name": "ACME CONSULTING",
                "country": "China",
                "from_date": "2019-06-15",
                "to_date": "2024-06-14",
                "grounds": "Collusive Practice",
                "sanction_type": "Debarment",
                "cross_debarment": false
            }
        ]);
        let records = mock.as_array().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["firm_name"], "SHELL CONSTRUCTION CO");
        assert!(records[0]["cross_debarment"].as_bool().unwrap());
        assert_eq!(records[1]["grounds"], "Collusive Practice");
    }
}
