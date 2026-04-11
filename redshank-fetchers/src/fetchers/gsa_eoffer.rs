//! GSA eOffer — GSA lease data for government-leased properties.
//!
//! API: <https://api.gsa.gov/acquisition/eoffer/>
//! Requires api.data.gov API key.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.gsa.gov/acquisition/lease-inventory/v1/leases";
const DEFAULT_LIMIT: u32 = 100;

/// Fetch GSA lease data.
///
/// # Arguments
///
/// * `query` - Search term for lessor name or address.
/// * `api_key` - GSA API key from api.data.gov.
/// * `output_dir` - Directory to write NDJSON output.
/// * `rate_limit_ms` - Delay between paginated requests.
/// * `max_pages` - Maximum number of pages to fetch (0 = unlimited).
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_leases(
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
        let offset = page * DEFAULT_LIMIT;

        let resp = client
            .get(API_BASE)
            .header("X-Api-Key", api_key)
            .query(&[
                ("q", query),
                ("limit", &DEFAULT_LIMIT.to_string()),
                ("offset", &offset.to_string()),
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
        let results = extract_leases(&json);

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        // Check if we've fetched all results
        let total = json
            .get("total")
            .or_else(|| json.get("totalCount"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if u64::from(offset + DEFAULT_LIMIT) >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("gsa_leases.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "gsa-eoffer".into(),
        attribution: None,
    })
}

/// Extract lease records from GSA response.
fn extract_leases(json: &serde_json::Value) -> Vec<serde_json::Value> {
    json.get("leases")
        .or_else(|| json.get("results"))
        .or_else(|| json.get("data"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Extracted GSA lease details.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaseDetails {
    /// Lessor (property owner) name.
    pub lessor: String,
    /// Property address.
    pub address: String,
    /// Square footage of leased space.
    pub square_feet: Option<u64>,
    /// Annual rent amount.
    pub annual_rent: Option<f64>,
    /// Lease expiration date.
    pub expiration_date: Option<String>,
}

/// Extract lease details from a GSA lease record.
#[must_use]
pub fn extract_lease_details(record: &serde_json::Value) -> Option<LeaseDetails> {
    let lessor = record
        .get("lessor")
        .or_else(|| record.get("lessorName"))
        .or_else(|| record.get("owner"))
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let address = build_address(record);
    if address.is_empty() {
        return None;
    }

    let square_feet = record
        .get("squareFeet")
        .or_else(|| record.get("rsf"))
        .or_else(|| record.get("rentable_square_feet"))
        .and_then(serde_json::Value::as_u64);

    let annual_rent = record
        .get("annualRent")
        .or_else(|| record.get("annual_rent"))
        .and_then(serde_json::Value::as_f64);

    let expiration_date = record
        .get("expirationDate")
        .or_else(|| record.get("lease_expiration_date"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    Some(LeaseDetails {
        lessor,
        address,
        square_feet,
        annual_rent,
        expiration_date,
    })
}

/// Build a formatted address from record fields.
fn build_address(record: &serde_json::Value) -> String {
    // Check for pre-formatted address field
    if let Some(addr) = record.get("address").and_then(serde_json::Value::as_str) {
        return addr.to_string();
    }

    // Build from components
    let street = record
        .get("street")
        .or_else(|| record.get("streetAddress"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    let city = record
        .get("city")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    let state = record
        .get("state")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    let zip = record
        .get("zip")
        .or_else(|| record.get("zipCode"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    let mut parts = Vec::new();
    if !street.is_empty() {
        parts.push(street.to_string());
    }
    if !city.is_empty() {
        parts.push(city.to_string());
    }
    if !state.is_empty() {
        parts.push(state.to_string());
    }
    if !zip.is_empty() {
        parts.push(zip.to_string());
    }

    parts.join(", ")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn gsa_parses_lease_response() {
        let mock_json = serde_json::json!({
            "total": 2,
            "leases": [
                {
                    "lessor": "ACME PROPERTIES LLC",
                    "street": "123 Main St",
                    "city": "Washington",
                    "state": "DC",
                    "zip": "20001",
                    "squareFeet": 50000,
                    "annualRent": 2_500_000.0,
                    "expirationDate": "2030-12-31"
                },
                {
                    "lessor": "FEDERAL REALTY INC",
                    "address": "456 Constitution Ave, Washington, DC 20002",
                    "squareFeet": 25000,
                    "annualRent": 1_200_000.0
                }
            ]
        });

        let leases = extract_leases(&mock_json);
        assert_eq!(leases.len(), 2);
        assert_eq!(leases[0]["lessor"], "ACME PROPERTIES LLC");
    }

    #[test]
    fn gsa_extracts_lease_details_with_components() {
        let record = serde_json::json!({
            "lessor": "PROPERTY OWNER INC",
            "street": "789 Independence Ave",
            "city": "Washington",
            "state": "DC",
            "zip": "20003",
            "squareFeet": 30000,
            "annualRent": 1_500_000.0,
            "expirationDate": "2028-06-30"
        });

        let details = extract_lease_details(&record).unwrap();
        assert_eq!(details.lessor, "PROPERTY OWNER INC");
        assert_eq!(
            details.address,
            "789 Independence Ave, Washington, DC, 20003"
        );
        assert_eq!(details.square_feet, Some(30000));
        assert!((details.annual_rent.unwrap() - 1_500_000.0).abs() < f64::EPSILON);
        assert_eq!(details.expiration_date, Some("2028-06-30".to_string()));
    }

    #[test]
    fn gsa_extracts_lease_details_with_formatted_address() {
        let record = serde_json::json!({
            "lessor": "DIRECT ADDRESS LLC",
            "address": "100 Pennsylvania Ave NW, Washington, DC 20500",
            "rsf": 100000,
            "annual_rent": 5_000_000.0
        });

        let details = extract_lease_details(&record).unwrap();
        assert_eq!(details.lessor, "DIRECT ADDRESS LLC");
        assert_eq!(
            details.address,
            "100 Pennsylvania Ave NW, Washington, DC 20500"
        );
        assert_eq!(details.square_feet, Some(100000));
    }

    #[test]
    fn gsa_handles_missing_optional_fields() {
        let record = serde_json::json!({
            "lessorName": "MINIMAL LESSOR",
            "streetAddress": "1 Test Street",
            "city": "Anytown",
            "state": "VA"
        });

        let details = extract_lease_details(&record).unwrap();
        assert_eq!(details.lessor, "MINIMAL LESSOR");
        assert!(details.square_feet.is_none());
        assert!(details.annual_rent.is_none());
        assert!(details.expiration_date.is_none());
    }
}
