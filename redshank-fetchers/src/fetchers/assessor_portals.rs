//! County assessor portal data — official property ownership and tax records.
//!
//! Parses JSON API responses and HTML from county assessor portals.
//! Supported counties: Cook (IL), Harris (TX), Maricopa (AZ), San Diego (CA), King (WA).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

/// Supported county assessor portals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum County {
    /// Cook County, IL (Chicago).
    Cook,
    /// Harris County, TX (Houston).
    Harris,
    /// Maricopa County, AZ (Phoenix).
    Maricopa,
    /// San Diego County, CA.
    SanDiego,
    /// King County, WA (Seattle).
    King,
}

impl County {
    /// Parse county from a string identifier.
    #[must_use]
    pub fn parse_county_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "cook" | "cook_il" | "chicago" => Some(Self::Cook),
            "harris" | "harris_tx" | "houston" => Some(Self::Harris),
            "maricopa" | "maricopa_az" | "phoenix" => Some(Self::Maricopa),
            "san_diego" | "sandiego" | "san diego" => Some(Self::SanDiego),
            "king" | "king_wa" | "seattle" => Some(Self::King),
            _ => None,
        }
    }

    /// Return the base URL for the county's assessor search.
    #[must_use]
    pub fn base_url(self) -> &'static str {
        match self {
            Self::Cook => "https://www.cookcountyassessor.com/api/search",
            Self::Harris => "https://hcad.org/api/property",
            Self::Maricopa => "https://mcassessor.maricopa.gov/api/property",
            Self::SanDiego => "https://arcc.sdcounty.ca.gov/api/property",
            Self::King => "https://blue.kingcounty.com/Assessor/eRealProperty/API",
        }
    }
}

/// An assessor property record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssessorRecord {
    /// Assessor Parcel Number (APN) — the official property identifier.
    pub apn: String,
    /// Property address.
    pub address: String,
    /// Owner of record (may be an LLC, trust, or individual).
    pub owner: Option<String>,
    /// Owner mailing address (often reveals beneficial owner's home address).
    pub mailing_address: Option<String>,
    /// Total assessed value in USD (land + improvements).
    pub assessed_value: Option<i64>,
    /// Land value component of the assessed value.
    pub land_value: Option<i64>,
    /// Improvement value component.
    pub improvement_value: Option<i64>,
    /// Annual property tax in USD.
    pub annual_tax: Option<i64>,
    /// Tax status ("Current", "Delinquent", "Exempt").
    pub tax_status: Option<String>,
    /// County where the property is located.
    pub county: String,
    /// State abbreviation.
    pub state: String,
    /// Recent deed transfer history.
    pub transfers: Vec<DeedTransfer>,
}

/// A recorded deed transfer event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeedTransfer {
    /// Transfer date (ISO 8601).
    pub date: String,
    /// Grantor (seller or previous owner).
    pub grantor: Option<String>,
    /// Grantee (buyer or new owner).
    pub grantee: Option<String>,
    /// Sale price, if disclosed.
    pub price: Option<i64>,
    /// Document number.
    pub doc_number: Option<String>,
}

/// Parse a Cook County assessor JSON response.
#[must_use]
pub fn parse_cook_county(json: &serde_json::Value) -> Vec<AssessorRecord> {
    let records = json
        .get("results")
        .or_else(|| json.get("data"))
        .and_then(serde_json::Value::as_array);

    records
        .map(|arr| {
            arr.iter()
                .filter_map(|item| parse_generic_assessor_record(item, "Cook", "IL"))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a generic county assessor JSON record.
///
/// Handles common field names used across county portals.
#[must_use]
pub fn parse_generic_assessor_record(
    item: &serde_json::Value,
    county: &str,
    state: &str,
) -> Option<AssessorRecord> {
    let str_field = |keys: &[&str]| -> Option<String> {
        keys.iter().find_map(|k| {
            item.get(*k)
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
    };

    let i64_field = |keys: &[&str]| -> Option<i64> {
        keys.iter()
            .find_map(|k| item.get(*k).and_then(serde_json::Value::as_i64))
    };

    let apn = str_field(&["apn", "pin", "parcel_number", "ParcelNumber", "parcelId"])?;
    let address = str_field(&["address", "property_address", "siteAddress", "SiteAddress"])
        .unwrap_or_default();

    let transfers = item
        .get("transfers")
        .or_else(|| item.get("deedHistory"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let date = t
                        .get("date")
                        .or_else(|| t.get("recordedDate"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)?;
                    Some(DeedTransfer {
                        date,
                        grantor: t
                            .get("grantor")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from),
                        grantee: t
                            .get("grantee")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from),
                        price: t
                            .get("price")
                            .or_else(|| t.get("salePrice"))
                            .and_then(serde_json::Value::as_i64),
                        doc_number: t
                            .get("docNumber")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(AssessorRecord {
        apn,
        address,
        owner: str_field(&["owner", "ownerName", "taxpayerName"]),
        mailing_address: str_field(&["mailing_address", "mailingAddress", "taxpayerAddress"]),
        assessed_value: i64_field(&["assessedValue", "totalAssessed", "totalValue"]),
        land_value: i64_field(&["landValue", "landAssessed"]),
        improvement_value: i64_field(&["improvementValue", "buildingValue"]),
        annual_tax: i64_field(&["annualTax", "taxAmount", "taxes"]),
        tax_status: str_field(&["taxStatus", "tax_status"]),
        county: county.to_string(),
        state: state.to_string(),
        transfers,
    })
}

/// Fetch assessor records for a county by address or APN.
///
/// # Errors
///
/// Returns `Err` if the county is unsupported, the HTTP request fails,
/// or the response cannot be parsed.
pub async fn fetch_assessor_records(
    county_str: &str,
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let county = County::parse_county_str(county_str).ok_or_else(|| {
        FetchError::Other(format!(
            "unsupported county '{county_str}'; supported: cook, harris, maricopa, san_diego, king"
        ))
    })?;

    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    let resp = client
        .get(county.base_url())
        .query(&[("q", query), ("search", query)])
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

    let (county_name, state_abbr) = match county {
        County::Cook => ("Cook", "IL"),
        County::Harris => ("Harris", "TX"),
        County::Maricopa => ("Maricopa", "AZ"),
        County::SanDiego => ("San Diego", "CA"),
        County::King => ("King", "WA"),
    };

    let records = parse_cook_county(&json)
        .into_iter()
        .map(|mut r| {
            r.county = county_name.to_string();
            r.state = state_abbr.to_string();
            r
        })
        .collect::<Vec<_>>();

    let serialized: Vec<serde_json::Value> = records
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let output_path = output_dir.join(format!(
        "assessor_{}.ndjson",
        county_str.to_ascii_lowercase().replace(' ', "_")
    ));
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "assessor_portal".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn cook_fixture() -> serde_json::Value {
        serde_json::json!({
            "results": [
                {
                    "pin": "12-34-567-890-1234",
                    "siteAddress": "1234 LAKESHORE DR",
                    "ownerName": "LAKESIDE TRUST LLC",
                    "mailingAddress": "9999 PRIVATE LANE, CHICAGO IL 60601",
                    "totalAssessed": 1_250_000,
                    "landValue": 500_000,
                    "improvementValue": 750_000,
                    "taxAmount": 28_500,
                    "taxStatus": "Current",
                    "transfers": [
                        {
                            "date": "2020-06-15",
                            "grantor": "JOHN SMITH",
                            "grantee": "LAKESIDE TRUST LLC",
                            "price": 1_800_000,
                            "docNumber": "2020-123456"
                        }
                    ]
                },
                {
                    "pin": "56-78-901-234-5678",
                    "siteAddress": "5678 MICHIGAN AVE",
                    "ownerName": "COMMERCIAL REALTY INC",
                    "totalAssessed": 5_500_000,
                    "taxStatus": "Delinquent",
                    "transfers": []
                }
            ]
        })
    }

    #[test]
    fn assessor_config_validates_for_supported_counties() {
        assert!(County::parse_county_str("cook").is_some());
        assert!(County::parse_county_str("harris").is_some());
        assert!(County::parse_county_str("maricopa").is_some());
        assert!(County::parse_county_str("san_diego").is_some());
        assert!(County::parse_county_str("king").is_some());
        assert!(County::parse_county_str("unknown_county").is_none());
    }

    #[test]
    fn assessor_extracts_apn_assessed_value_owner_and_tax_status() {
        let json = cook_fixture();
        let records = parse_cook_county(&json);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].apn, "12-34-567-890-1234");
        assert_eq!(records[0].address, "1234 LAKESHORE DR");
        assert_eq!(records[0].owner.as_deref(), Some("LAKESIDE TRUST LLC"));
        assert_eq!(records[0].assessed_value, Some(1_250_000));
        assert_eq!(records[0].tax_status.as_deref(), Some("Current"));
    }

    #[test]
    fn assessor_extracts_mailing_address_revealing_beneficial_owner() {
        let json = cook_fixture();
        let records = parse_cook_county(&json);

        assert_eq!(
            records[0].mailing_address.as_deref(),
            Some("9999 PRIVATE LANE, CHICAGO IL 60601")
        );
    }

    #[test]
    fn assessor_extracts_deed_transfer_history() {
        let json = cook_fixture();
        let records = parse_cook_county(&json);

        assert_eq!(records[0].transfers.len(), 1);
        assert_eq!(records[0].transfers[0].date, "2020-06-15");
        assert_eq!(
            records[0].transfers[0].grantor.as_deref(),
            Some("JOHN SMITH")
        );
        assert_eq!(
            records[0].transfers[0].grantee.as_deref(),
            Some("LAKESIDE TRUST LLC")
        );
        assert_eq!(records[0].transfers[0].price, Some(1_800_000));
    }

    #[test]
    fn assessor_handles_delinquent_tax_status() {
        let json = cook_fixture();
        let records = parse_cook_county(&json);

        assert_eq!(records[1].tax_status.as_deref(), Some("Delinquent"));
    }
}
