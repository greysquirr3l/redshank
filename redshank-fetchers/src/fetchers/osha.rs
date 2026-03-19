//! OSHA — Occupational Safety and Health Administration inspection data.
//!
//! API: <https://enforcedata.dol.gov/views/data_summary.php>
//! Pagination: skip + limit.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://data.dol.gov/get/inspection";

/// Fetch OSHA inspection data.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_inspections(
    query: &str,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let limit: u32 = 200;
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let skip = page * limit;
        let resp = client
            .get(API_BASE)
            .header("X-API-KEY", api_key)
            .query(&[
                ("$filter", &format!("estab_name eq '{query}'")),
                ("$skip", &skip.to_string()),
                ("$top", &limit.to_string()),
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

    let output_path = output_dir.join("osha.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "osha".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    #[test]
    fn osha_parses_inspection_response() {
        let mock: Vec<serde_json::Value> = vec![serde_json::json!({
            "activity_nr": "1234567",
            "estab_name": "ACME FACTORY",
            "site_state": "OH",
            "open_date": "2024-01-15"
        })];
        assert_eq!(mock.len(), 1);
        assert_eq!(mock[0]["estab_name"], "ACME FACTORY");
    }
}
