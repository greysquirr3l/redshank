//! Extended NPI registry parsing helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://npiregistry.cms.hhs.gov/api/";

/// A taxonomy entry for an NPI record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct NpiTaxonomy {
    pub code: String,
    pub desc: Option<String>,
    pub primary: bool,
}

/// An address entry for an NPI record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct NpiPracticeLocation {
    pub purpose: Option<String>,
    pub address_1: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
}

/// Extended organization/provider details from the NPI registry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ExtendedNpiRecord {
    pub npi: String,
    pub name: String,
    pub parent_organization_npi: Option<String>,
    pub authorized_official_name: Option<String>,
    pub authorized_official_title: Option<String>,
    pub authorized_official_credential: Option<String>,
    pub enumeration_date: Option<String>,
    pub last_update_date: Option<String>,
    pub taxonomies: Vec<NpiTaxonomy>,
    pub practice_locations: Vec<NpiPracticeLocation>,
}

fn optional_string(json: &serde_json::Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn extract_name(record: &serde_json::Value) -> Option<String> {
    if let Some(org_name) = record
        .get("basic")
        .and_then(|basic| basic.get("organization_name"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(org_name.to_string());
    }

    let basic = record.get("basic")?;
    let first_name = basic
        .get("first_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let last_name = basic
        .get("last_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    let full = format!("{first_name} {last_name}").trim().to_string();
    (!full.is_empty()).then_some(full)
}

/// Parse an extended NPI record.
#[must_use]
pub fn parse_extended_record(record: &serde_json::Value) -> Option<ExtendedNpiRecord> {
    let npi = record.get("number").and_then(|value| {
        value
            .as_u64()
            .map(|number| number.to_string())
            .or_else(|| value.as_str().map(str::to_string))
    })?;
    let name = extract_name(record)?;
    let basic = record.get("basic").unwrap_or(&serde_json::Value::Null);

    let authorized_official_name = [
        optional_string(basic, "authorized_official_first_name"),
        optional_string(basic, "authorized_official_last_name"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");

    let taxonomies = record
        .get("taxonomies")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(NpiTaxonomy {
                        code: optional_string(item, "code")?,
                        desc: optional_string(item, "desc"),
                        primary: item
                            .get("primary")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let practice_locations = record
        .get("addresses")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| NpiPracticeLocation {
                    purpose: optional_string(item, "address_purpose"),
                    address_1: optional_string(item, "address_1"),
                    city: optional_string(item, "city"),
                    state: optional_string(item, "state"),
                    postal_code: optional_string(item, "postal_code"),
                })
                .collect()
        })
        .unwrap_or_default();

    Some(ExtendedNpiRecord {
        npi,
        name,
        parent_organization_npi: optional_string(basic, "parent_organization_lbn"),
        authorized_official_name: (!authorized_official_name.is_empty())
            .then_some(authorized_official_name),
        authorized_official_title: optional_string(basic, "authorized_official_title_or_position"),
        authorized_official_credential: optional_string(basic, "authorized_official_credential"),
        enumeration_date: optional_string(basic, "enumeration_date"),
        last_update_date: optional_string(basic, "last_updated"),
        taxonomies,
        practice_locations,
    })
}

/// Fetch extended NPI details by number.
///
/// # Errors
///
/// Returns `Err` if the request fails or the server returns a non-success status.
pub async fn fetch_by_npi(npi: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get(API_BASE)
        .query(&[("version", "2.1"), ("number", npi), ("pretty", "true")])
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
    let records = json
        .get("results")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(parse_extended_record)
                .map(|record| {
                    serde_json::to_value(record).map_err(|err| FetchError::Parse(err.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();

    let output_path = output_dir.join("npi_extended.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "npi-extended".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn organization_fixture() -> serde_json::Value {
        serde_json::json!({
            "number": 1_111_222_233_u64,
            "basic": {
                "organization_name": "North Valley Cardiology Group",
                "parent_organization_lbn": "9988776655",
                "authorized_official_first_name": "Amira",
                "authorized_official_last_name": "Rahman",
                "authorized_official_title_or_position": "Chief Compliance Officer",
                "authorized_official_credential": "JD",
                "enumeration_date": "2017-01-12",
                "last_updated": "2024-03-08"
            },
            "taxonomies": [
                {"code": "207RC0000X", "desc": "Cardiovascular Disease Physician", "primary": true},
                {"code": "261QC0050X", "desc": "Clinic/Center Cardiology", "primary": false}
            ],
            "addresses": [
                {"address_purpose": "LOCATION", "address_1": "100 Main St", "city": "Austin", "state": "TX", "postal_code": "78701"},
                {"address_purpose": "MAILING", "address_1": "PO Box 101", "city": "Austin", "state": "TX", "postal_code": "78767"}
            ]
        })
    }

    #[test]
    fn npi_extended_parses_organizational_hierarchy() {
        let record = parse_extended_record(&organization_fixture()).unwrap();

        assert_eq!(record.name, "North Valley Cardiology Group");
        assert_eq!(
            record.parent_organization_npi.as_deref(),
            Some("9988776655")
        );
    }

    #[test]
    fn npi_extended_extracts_authorized_official_and_taxonomies() {
        let record = parse_extended_record(&organization_fixture()).unwrap();

        assert_eq!(
            record.authorized_official_name.as_deref(),
            Some("Amira Rahman")
        );
        assert_eq!(
            record.authorized_official_title.as_deref(),
            Some("Chief Compliance Officer")
        );
        assert_eq!(record.taxonomies.len(), 2);
        assert_eq!(record.taxonomies[0].code, "207RC0000X");
        assert!(record.taxonomies[0].primary);
    }
}
