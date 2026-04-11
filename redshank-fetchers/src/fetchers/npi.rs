//! NPI — National Provider Identifier Registry for healthcare provider lookup.
//!
//! API: <https://npiregistry.cms.hhs.gov/api/>
//! No authentication required.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://npiregistry.cms.hhs.gov/api/";
const DEFAULT_LIMIT: u32 = 200;

/// Fetch NPI provider data by NPI number.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_by_npi(npi: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    let resp = client
        .get(API_BASE)
        .query(&[("version", "2.1"), ("search_type", "NPI"), ("number", npi)])
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

    let output_path = output_dir.join("npi_provider.ndjson");
    let count = write_ndjson(&output_path, &results)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "npi".into(),
        attribution: None,
    })
}

/// Fetch NPI providers by name search.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_by_name(
    first_name: Option<&str>,
    last_name: Option<&str>,
    organization_name: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for skip in (0..).step_by(DEFAULT_LIMIT as usize).take(max as usize) {
        let mut query: Vec<(&str, String)> = vec![
            ("version", "2.1".to_string()),
            ("limit", DEFAULT_LIMIT.to_string()),
        ];

        if skip > 0 {
            query.push(("skip", skip.to_string()));
        }

        if let Some(fn_) = first_name {
            query.push(("first_name", fn_.to_string()));
        }
        if let Some(ln) = last_name {
            query.push(("last_name", ln.to_string()));
        }
        if let Some(org) = organization_name {
            query.push(("organization_name", org.to_string()));
        }

        let query_refs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let resp = client.get(API_BASE).query(&query_refs).send().await?;

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

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        let result_count = json
            .get("result_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if u64::try_from(skip).unwrap_or(u64::MAX) + u64::from(DEFAULT_LIMIT) >= result_count {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("npi_providers.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "npi".into(),
        attribution: None,
    })
}

/// Extract results from NPI response.
fn extract_results(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("results")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Extracted provider details from NPI data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDetails {
    /// 10-digit NPI number.
    pub npi: String,
    /// Provider name (individual or organization).
    pub name: String,
    /// Provider taxonomy/specialty code.
    pub taxonomy: Option<String>,
    /// State where provider is located.
    pub state: Option<String>,
    /// City where provider is located.
    pub city: Option<String>,
}

/// Extract provider details from an NPI record.
#[must_use]
pub fn extract_provider_details(record: &serde_json::Value) -> Option<ProviderDetails> {
    let npi = record
        .get("number")
        .and_then(serde_json::Value::as_u64)
        .map(|n| n.to_string())
        .or_else(|| {
            record
                .get("number")
                .and_then(serde_json::Value::as_str)
                .map(String::from)
        })?;

    let name = extract_provider_name(record)?;

    let taxonomy = record
        .get("taxonomies")
        .and_then(serde_json::Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|t| t.get("code"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let addresses = record
        .get("addresses")
        .and_then(serde_json::Value::as_array);
    let primary_address = addresses.and_then(|arr| {
        arr.iter()
            .find(|a| {
                a.get("address_purpose").and_then(serde_json::Value::as_str) == Some("LOCATION")
            })
            .or_else(|| arr.first())
    });

    let state = primary_address
        .and_then(|a| a.get("state"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let city = primary_address
        .and_then(|a| a.get("city"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    Some(ProviderDetails {
        npi,
        name,
        taxonomy,
        state,
        city,
    })
}

/// Extract provider name from either individual or organization fields.
fn extract_provider_name(record: &serde_json::Value) -> Option<String> {
    // Try organization name first
    if let Some(org) = record
        .get("basic")
        .and_then(|b| b.get("organization_name"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(org.to_string());
    }

    // Fall back to individual name
    let basic = record.get("basic")?;
    let first = basic
        .get("first_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let last = basic
        .get("last_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    if first.is_empty() && last.is_empty() {
        return None;
    }

    Some(format!("{first} {last}").trim().to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn npi_parses_provider_response() {
        let mock_json = serde_json::json!({
            "result_count": 2,
            "results": [
                {
                    "number": 1234567890_u64,
                    "basic": {
                        "first_name": "JOHN",
                        "last_name": "DOE"
                    },
                    "taxonomies": [
                        {"code": "207Q00000X", "desc": "Family Medicine"}
                    ],
                    "addresses": [
                        {"address_purpose": "LOCATION", "city": "CHICAGO", "state": "IL"}
                    ]
                },
                {
                    "number": 9876543210_u64,
                    "basic": {
                        "organization_name": "ACME HOSPITAL"
                    },
                    "taxonomies": [
                        {"code": "282N00000X", "desc": "General Acute Care Hospital"}
                    ],
                    "addresses": [
                        {"address_purpose": "LOCATION", "city": "NEW YORK", "state": "NY"}
                    ]
                }
            ]
        });

        let results = extract_results(&mock_json);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn npi_extracts_individual_provider_details() {
        let record = serde_json::json!({
            "number": 1234567890_u64,
            "basic": {
                "first_name": "JOHN",
                "last_name": "DOE"
            },
            "taxonomies": [
                {"code": "207Q00000X", "desc": "Family Medicine"}
            ],
            "addresses": [
                {"address_purpose": "LOCATION", "city": "CHICAGO", "state": "IL"}
            ]
        });

        let details = extract_provider_details(&record).unwrap();
        assert_eq!(details.npi, "1234567890");
        assert_eq!(details.name, "JOHN DOE");
        assert_eq!(details.taxonomy, Some("207Q00000X".to_string()));
        assert_eq!(details.state, Some("IL".to_string()));
        assert_eq!(details.city, Some("CHICAGO".to_string()));
    }

    #[test]
    fn npi_extracts_organization_provider_details() {
        let record = serde_json::json!({
            "number": 9876543210_u64,
            "basic": {
                "organization_name": "ACME HOSPITAL"
            },
            "taxonomies": [
                {"code": "282N00000X"}
            ],
            "addresses": [
                {"address_purpose": "MAILING", "city": "NEW YORK", "state": "NY"},
                {"address_purpose": "LOCATION", "city": "BROOKLYN", "state": "NY"}
            ]
        });

        let details = extract_provider_details(&record).unwrap();
        assert_eq!(details.npi, "9876543210");
        assert_eq!(details.name, "ACME HOSPITAL");
        // Should pick LOCATION address over MAILING
        assert_eq!(details.city, Some("BROOKLYN".to_string()));
    }
}
