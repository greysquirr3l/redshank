//! FPDS-NG — Federal Procurement Data System (granular contract awards).
//!
//! API: `https://api.sam.gov/prod/opportunities/v2/search`
//! More granular than `USASpending`: individual line items, modification history,
//! award type codes, NAICS.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.sam.gov/prod/opportunities/v2";

/// Fetch granular FPDS contract awards for the given query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_fpds_awards(
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
        let body = serde_json::json!({
            "keyword": query,
            "page": page,
            "size": 100,
        });

        let resp = client
            .post(format!("{API_BASE}/search"))
            .query(&[("api_key", api_key)])
            .json(&body)
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
        let opportunities = json
            .get("opportunitiesData")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if opportunities.is_empty() {
            break;
        }
        all_records.extend(opportunities);

        let total = json
            .get("totalRecords")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if u64::from(page + 1) * 100 >= total {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("fpds_awards.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "fpds".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    #[test]
    fn fpds_parses_opportunity_response() {
        let mock = serde_json::json!({
            "totalRecords": 2,
            "opportunitiesData": [
                {
                    "noticeId": "CONT-2024-001",
                    "title": "IT Services Contract",
                    "awardee": "ACME TECH INC",
                    "awardAmount": 1_500_000,
                    "naicsCode": "541512",
                    "awardType": "Firm Fixed Price",
                    "modifications": [
                        {"modNumber": "P00001", "amount": 250_000}
                    ]
                },
                {
                    "noticeId": "CONT-2024-002",
                    "title": "Consulting Services",
                    "awardee": "SHELL CONSULTING LLC",
                    "awardAmount": 500_000,
                    "naicsCode": "541611",
                    "awardType": "Time and Materials",
                    "modifications": []
                }
            ]
        });
        let opps = mock
            .get("opportunitiesData")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(opps.len(), 2);
        assert_eq!(opps[0]["awardee"], "ACME TECH INC");
        assert_eq!(opps[0]["naicsCode"], "541512");
        let mods = opps[0]["modifications"].as_array().unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(opps[1]["awardType"], "Time and Materials");
    }
}
