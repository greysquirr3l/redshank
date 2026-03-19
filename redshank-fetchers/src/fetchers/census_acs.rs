//! Census ACS — American Community Survey data.
//!
//! API: <https://api.census.gov/data/{year}/acs/acs5>
//! No pagination — single request returns all matching rows.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// Fetch Census ACS data for a given variable set and geography.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_acs(
    year: u32,
    variables: &str,
    geography: &str,
    api_key: Option<&str>,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let base = format!("https://api.census.gov/data/{year}/acs/acs5");

    let mut params = vec![
        ("get", variables.to_string()),
        ("for", geography.to_string()),
    ];
    if let Some(key) = api_key {
        params.push(("key", key.to_string()));
    }

    let resp = client.get(&base).query(&params).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    // Census API returns array-of-arrays: first row is headers.
    let json: serde_json::Value = resp.json().await?;
    let records = parse_census_response(&json);

    let output_path = output_dir.join("census_acs.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "census-acs".into(),
    })
}

/// Convert Census array-of-arrays response into record objects.
#[must_use]
pub fn parse_census_response(json: &serde_json::Value) -> Vec<serde_json::Value> {
    let rows = match json.as_array() {
        Some(r) if r.len() >= 2 => r,
        _ => return Vec::new(),
    };

    let headers: Vec<&str> = rows
        .first()
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    rows.get(1..)
        .unwrap_or_default()
        .iter()
        .filter_map(|row| {
            let values = row.as_array()?;
            let mut record = serde_json::Map::new();
            for (i, header) in headers.iter().enumerate() {
                if let Some(val) = values.get(i) {
                    record.insert((*header).to_string(), val.clone());
                }
            }
            Some(serde_json::Value::Object(record))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn census_response_parsed_into_records() {
        let response = serde_json::json!([
            ["NAME", "B01001_001E", "state"],
            ["Alabama", "5024279", "01"],
            ["Alaska", "733391", "02"],
        ]);
        let records = parse_census_response(&response);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["NAME"], "Alabama");
        assert_eq!(records[0]["B01001_001E"], "5024279");
        assert_eq!(records[1]["state"], "02");
    }
}
