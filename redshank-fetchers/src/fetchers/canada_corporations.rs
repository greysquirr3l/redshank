//! Canada Corporations — federal corporate registry via Corporations Canada CKAN API.
//!
//! Source: <https://open.canada.ca/data/en/dataset/> — Corporations Canada databases.
//! No authentication required. Covers federally incorporated corporations only.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const CKAN_API: &str = "https://open.canada.ca/data/api/3/action/datastore_search";

/// Federal corporation search resource ID for active/historical records.
const CORPS_RESOURCE_ID: &str = "0005dc42-c26c-4e5e-9eca-7a7d91a22406";

/// A Canadian federal corporation record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorpRecord {
    /// Corporation number (unique federal identifier).
    pub corporation_number: String,
    /// Legal corporate name.
    pub corporation_name: String,
    /// Corporation status (active, dissolved, etc.).
    pub status: Option<String>,
    /// Date of incorporation.
    pub date_of_incorporation: Option<String>,
    /// Registered head office province/territory.
    pub registered_office_province: Option<String>,
    /// Revenue Canada tax number (BN), if available.
    pub business_number: Option<String>,
}

/// Fetch federal corporations matching `query` via Corporations Canada CKAN API.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or returns a non-2xx status.
pub async fn fetch_corporations(
    query: &str,
    output_dir: &Path,
    max_results: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let limit = max_results.min(100);

    let resp = client
        .get(CKAN_API)
        .query(&[
            ("resource_id", CORPS_RESOURCE_ID),
            ("q", query),
            ("limit", &limit.to_string()),
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
    let records = parse_corporation_results(&json);
    let serialized: Vec<serde_json::Value> = records
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let output_path = output_dir.join("canada_corporations.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "canada_corporations".into(),
        attribution: None,
    })
}

/// Parse corporation records from a CKAN `datastore_search` API response.
#[must_use]
pub fn parse_corporation_results(json: &serde_json::Value) -> Vec<CorpRecord> {
    json.get("result")
        .and_then(|r| r.get("records"))
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().map(parse_corp_item).collect())
        .unwrap_or_default()
}

fn parse_corp_item(item: &serde_json::Value) -> CorpRecord {
    // CKAN field names vary by dataset version; try both snake_case and PascalCase
    let corporation_number = item
        .get("corporation_number")
        .or_else(|| item.get("corp_num"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    let corporation_name = item
        .get("corporation_name")
        .or_else(|| item.get("corp_nm"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    let status = item
        .get("status")
        .or_else(|| item.get("corp_status_nm"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let date_of_incorporation = item
        .get("date_of_incorporation")
        .or_else(|| item.get("incorporation_dt"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let registered_office_province = item
        .get("registered_office_province")
        .or_else(|| item.get("reg_office_prov_nm"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let business_number = item
        .get("business_number")
        .or_else(|| item.get("bn"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    CorpRecord {
        corporation_number,
        corporation_name,
        status,
        date_of_incorporation,
        registered_office_province,
        business_number,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn ckan_fixture() -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "result": {
                "total": 3,
                "records": [
                    {
                        "corporation_number": "6789012",
                        "corporation_name": "Maple Holdings Inc.",
                        "status": "Active",
                        "date_of_incorporation": "1998-04-15",
                        "registered_office_province": "Ontario",
                        "business_number": "123456789"
                    },
                    {
                        "corp_num": "9876543",
                        "corp_nm": "Cedar Forest Corp.",
                        "corp_status_nm": "Dissolved",
                        "incorporation_dt": "2001-11-22",
                        "reg_office_prov_nm": "British Columbia"
                    },
                    {
                        "corporation_number": "1111111",
                        "corporation_name": "Northern Ventures Ltd.",
                        "status": "Active",
                        "date_of_incorporation": "2010-01-01"
                    }
                ]
            }
        })
    }

    #[test]
    fn canada_corps_parses_ckan_response_and_extracts_number_name_status() {
        let json = ckan_fixture();
        let records = parse_corporation_results(&json);

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].corporation_number, "6789012");
        assert_eq!(records[0].corporation_name, "Maple Holdings Inc.");
        assert_eq!(records[0].status.as_deref(), Some("Active"));
        assert_eq!(
            records[0].date_of_incorporation.as_deref(),
            Some("1998-04-15")
        );
        assert_eq!(
            records[0].registered_office_province.as_deref(),
            Some("Ontario")
        );
        assert_eq!(records[0].business_number.as_deref(), Some("123456789"));
    }

    #[test]
    fn canada_corps_handles_alternate_field_names() {
        let json = ckan_fixture();
        let records = parse_corporation_results(&json);

        // Second record uses alternate field names (corp_num, corp_nm, etc.)
        assert_eq!(records[1].corporation_number, "9876543");
        assert_eq!(records[1].corporation_name, "Cedar Forest Corp.");
        assert_eq!(records[1].status.as_deref(), Some("Dissolved"));
        assert_eq!(
            records[1].registered_office_province.as_deref(),
            Some("British Columbia")
        );
    }

    #[test]
    fn canada_corps_handles_empty_result() {
        let json = serde_json::json!({ "success": true, "result": { "total": 0, "records": [] } });
        let records = parse_corporation_results(&json);
        assert!(records.is_empty());
    }
}
