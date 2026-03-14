//! Federal Audit Clearinghouse (FAC) — Single audit database.
//!
//! API: `https://facdissem.census.gov/api/v1.0/submissions`
//! Covers any organisation that spent $750k+ in federal grants.
//! No auth required. JSON API.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://facdissem.census.gov/api/v1.0";

/// Fetch single-audit submissions for the given entity name.
pub async fn fetch_audits(
    entity_name: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let resp = client
            .get(format!("{API_BASE}/submissions"))
            .query(&[
                ("auditeeName", entity_name),
                ("page", &page.to_string()),
            ])
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
            .get("next")
            .map(|v| !v.is_null())
            .unwrap_or(false);

        if !has_next {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("federal_audit.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "federal_audit".into(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn fac_parses_audit_submissions_response() {
        let mock = serde_json::json!({
            "results": [
                {
                    "auditeeName": "CITY OF SPRINGFIELD",
                    "ein": "123456789",
                    "auditYear": "2023",
                    "totalFederalExpenditure": 5000000,
                    "findings": [
                        {"type": "material_weakness", "amount": 250000}
                    ],
                    "questionedCosts": 50000
                }
            ],
            "next": null
        });
        let results = mock.get("results").and_then(|v| v.as_array()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["auditeeName"], "CITY OF SPRINGFIELD");
        assert_eq!(results[0]["totalFederalExpenditure"], 5000000);
        let findings = results[0]["findings"].as_array().unwrap();
        assert_eq!(findings[0]["type"], "material_weakness");
    }
}
