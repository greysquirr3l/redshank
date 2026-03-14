//! SAM.gov — System for Award Management entity data.
//!
//! API: <https://api.sam.gov/entity-information/v3/entities>
//! Pagination: page-based (0-indexed), size max 10.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.sam.gov/entity-information/v3/entities";

/// Fetch SAM.gov entity data.
pub async fn fetch_entities(
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
        let resp = client
            .get(API_BASE)
            .query(&[
                ("api_key", api_key),
                ("legalBusinessName", query),
                ("page", &page.to_string()),
                ("size", "10"),
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
        let entities = json
            .get("entityData")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if entities.is_empty() {
            break;
        }
        all_records.extend(entities);

        let total = json
            .get("totalRecords")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        if all_records.len() >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("sam_gov.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "sam-gov".into(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn sam_gov_parses_entity_response() {
        let mock = serde_json::json!({
            "totalRecords": 1,
            "entityData": [
                {
                    "entityRegistration": {
                        "legalBusinessName": "ACME CORP",
                        "ueiSAM": "ABC123DEF456",
                        "registrationStatus": "Active"
                    }
                }
            ]
        });
        let entities = mock["entityData"].as_array().unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(
            entities[0]["entityRegistration"]["legalBusinessName"],
            "ACME CORP"
        );
    }
}
