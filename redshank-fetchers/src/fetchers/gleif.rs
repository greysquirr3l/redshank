//! GLEIF — Legal Entity Identifier (LEI) Registry.
//!
//! API: `https://api.gleif.org/api/v1/lei-records`
//! No auth required. Rate limit: 60 req/min.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.gleif.org/api/v1";
/// GLEIF allows 60 requests per minute.
const GLEIF_RATE_LIMIT_MS: u64 = 1000;

/// Fetch LEI records matching the given legal name.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_lei_records(
    legal_name: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let rate = rate_limit_ms.max(GLEIF_RATE_LIMIT_MS);
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let resp = client
            .get(format!("{API_BASE}/lei-records"))
            .query(&[
                ("filter[entity.legalName]", legal_name),
                ("page[number]", &page.to_string()),
                ("page[size]", "100"),
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
        let data = json
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if data.is_empty() {
            break;
        }
        all_records.extend(data);

        // GLEIF uses JSON:API pagination with meta.pagination
        let total_pages = u32::try_from(
            json.get("meta")
                .and_then(|m| m.get("pagination"))
                .and_then(|p| p.get("lastPage"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1),
        )
        .unwrap_or(u32::MAX);

        if page >= total_pages {
            break;
        }
        rate_limit_delay(rate).await;
    }

    let output_path = output_dir.join("gleif_lei.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "gleif".into(),
        attribution: None,
    })
}

/// Extract LEI, legal name, and parent LEI from a GLEIF data record.
pub fn extract_lei_fields(record: &serde_json::Value) -> Option<(String, String, Option<String>)> {
    let lei = record.get("id")?.as_str()?.to_owned();
    let legal_name = record
        .get("attributes")
        .and_then(|a| a.get("entity"))
        .and_then(|e| e.get("legalName"))
        .and_then(|n| n.get("name"))
        .and_then(|v| v.as_str())?
        .to_owned();
    let parent_lei = record
        .get("attributes")
        .and_then(|a| a.get("entity"))
        .and_then(|e| e.get("associatedEntity"))
        .and_then(|ae| ae.get("lei"))
        .and_then(|v| v.as_str())
        .map(String::from);
    Some((lei, legal_name, parent_lei))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn gleif_extracts_lei_legal_name_and_parent() {
        let record = serde_json::json!({
            "id": "529900T8BM49AURSDO55",
            "attributes": {
                "entity": {
                    "legalName": {"name": "ACME CORPORATION"},
                    "associatedEntity": {"lei": "PARENT123456789012345"}
                }
            }
        });
        let (lei, name, parent) = extract_lei_fields(&record).unwrap();
        assert_eq!(lei, "529900T8BM49AURSDO55");
        assert_eq!(name, "ACME CORPORATION");
        assert_eq!(parent.unwrap(), "PARENT123456789012345");
    }

    #[test]
    fn gleif_handles_no_parent_lei() {
        let record = serde_json::json!({
            "id": "529900T8BM49AURSDO55",
            "attributes": {
                "entity": {
                    "legalName": {"name": "STANDALONE INC"}
                }
            }
        });
        let (_, name, parent) = extract_lei_fields(&record).unwrap();
        assert_eq!(name, "STANDALONE INC");
        assert!(parent.is_none());
    }
}
