//! Maritime AIS (Automatic Identification System) — vessel tracking data.
//!
//! Primary source: VesselFinder API <https://www.vesselfinder.com/api>
//! Fallback: ITU MARS ship station database.
//!
//! Requires a VesselFinder API key for vessel lookups.
//! For OSINT: cross-reference vessel owners with sanctions lists, check for
//! AIS gaps (transponder off) as indicators of sanctions evasion, and flag
//! yachts > 30m as wealth indicators.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const VESSEL_FINDER_BASE: &str = "https://api.vesselfinder.com";

/// A maritime vessel record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VesselRecord {
    /// MMSI — Maritime Mobile Service Identity (9-digit unique identifier).
    pub mmsi: String,
    /// IMO number (permanent 7-digit identifier; may be absent for small vessels).
    pub imo_number: Option<String>,
    /// Vessel name.
    pub vessel_name: String,
    /// Call sign.
    pub call_sign: Option<String>,
    /// Flag state (country of registration).
    pub flag: Option<String>,
    /// Vessel type (cargo, tanker, yacht, fishing, etc.).
    pub vessel_type: Option<String>,
    /// Gross tonnage.
    pub gross_tonnage: Option<u32>,
    /// Deadweight tonnage.
    pub deadweight: Option<u32>,
    /// Year built.
    pub year_built: Option<u32>,
    /// Registered owner, if available.
    pub owner: Option<String>,
    /// Ship operator / manager.
    pub operator: Option<String>,
    /// Last known latitude.
    pub last_lat: Option<f64>,
    /// Last known longitude.
    pub last_lon: Option<f64>,
    /// Last known position timestamp (UTC).
    pub last_position_time: Option<String>,
    /// Last known port of call.
    pub last_port: Option<String>,
    /// Destination.
    pub destination: Option<String>,
}

/// Fetch vessel information by MMSI or vessel name.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or returns a non-2xx status.
pub async fn fetch_vessels(
    query: &str,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_results: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let limit = max_results.min(100);

    // Try direct MMSI lookup if the query looks like a 9-digit number
    if query.chars().all(char::is_numeric) && query.len() == 9 {
        rate_limit_delay(rate_limit_ms).await;
        if let Some(record) = fetch_by_mmsi(&client, api_key, query).await? {
            let v = serde_json::to_value(&record).map_err(|e| FetchError::Parse(e.to_string()))?;
            all_records.push(v);
        }
    } else {
        // Name search
        let resp = client
            .get(format!("{VESSEL_FINDER_BASE}/vessels"))
            .header("Authorization", format!("Bearer {api_key}"))
            .query(&[("name", query), ("limit", &limit.to_string())])
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
        let records = parse_vessel_list(&json);
        all_records.extend(records.iter().filter_map(|r| serde_json::to_value(r).ok()));
    }

    let output_path = output_dir.join("maritime_ais.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "maritime_ais".into(),
        attribution: None,
    })
}

async fn fetch_by_mmsi(
    client: &reqwest::Client,
    api_key: &str,
    mmsi: &str,
) -> Result<Option<VesselRecord>, FetchError> {
    let resp = client
        .get(format!("{VESSEL_FINDER_BASE}/vessels/{mmsi}"))
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await?;

    if resp.status().as_u16() == 404 {
        return Ok(None);
    }

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    Ok(parse_vessel_item(&json))
}

/// Parse a vessel list response from VesselFinder API.
///
/// Handles both `{"vessels": [...]}` and bare array `[...]` formats.
#[must_use]
pub fn parse_vessel_list(json: &serde_json::Value) -> Vec<VesselRecord> {
    let arr = json
        .get("vessels")
        .and_then(serde_json::Value::as_array)
        .or_else(|| json.as_array());

    arr.map(|items| items.iter().filter_map(parse_vessel_item).collect())
        .unwrap_or_default()
}

/// Parse a single vessel item.
#[must_use]
pub fn parse_vessel_item(item: &serde_json::Value) -> Option<VesselRecord> {
    let mmsi = item
        .get("mmsi")
        .or_else(|| item.get("MMSI"))
        .and_then(|v| {
            // MMSI may come as a string or number
            v.as_str()
                .map(String::from)
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        })?;

    let str_field = |key: &str, alt: &str| -> Option<String> {
        item.get(key)
            .or_else(|| item.get(alt))
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };

    let u32_field = |key: &str, alt: &str| -> Option<u32> {
        item.get(key)
            .or_else(|| item.get(alt))
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32)
    };

    let f64_field = |obj: &serde_json::Value, key: &str, alt: &str| -> Option<f64> {
        obj.get(key)
            .or_else(|| obj.get(alt))
            .and_then(serde_json::Value::as_f64)
    };

    // Position may be nested under "AIS" or "position" sub-object
    let pos = item
        .get("AIS")
        .or_else(|| item.get("position"))
        .unwrap_or(item);

    Some(VesselRecord {
        mmsi,
        imo_number: str_field("imo", "IMO"),
        vessel_name: str_field("name", "NAME").unwrap_or_default(),
        call_sign: str_field("callsign", "CALLSIGN"),
        flag: str_field("flag", "FLAG"),
        vessel_type: str_field("vessel_type", "TYPE_NAME"),
        gross_tonnage: u32_field("gross_tonnage", "GT"),
        deadweight: u32_field("deadweight", "DWT"),
        year_built: u32_field("year_built", "YEAR_BUILT"),
        owner: str_field("owner", "SHIPOWNER"),
        operator: str_field("operator", "MANAGER"),
        last_lat: f64_field(pos, "lat", "LATITUDE"),
        last_lon: f64_field(pos, "lng", "LONGITUDE")
            .or_else(|| pos.get("lon").and_then(serde_json::Value::as_f64)),
        last_position_time: str_field("timestamp", "TIMESTAMP"),
        last_port: str_field("last_port", "LAST_PORT"),
        destination: str_field("destination", "DESTINATION"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn vessel_search_fixture() -> serde_json::Value {
        serde_json::json!({
            "vessels": [
                {
                    "mmsi": "123456789",
                    "imo": "1234567",
                    "name": "NORTHERN STAR",
                    "callsign": "ABCD1",
                    "flag": "PA",
                    "vessel_type": "Yacht",
                    "gross_tonnage": 450,
                    "year_built": 2018,
                    "owner": "Offshore Holdings Ltd",
                    "lat": 25.7617,
                    "lng": -80.1918,
                    "timestamp": "2024-01-15T12:30:00Z",
                    "destination": "MIAMI"
                },
                {
                    "mmsi": "987654321",
                    "imo": "9876543",
                    "name": "PACIFIC CHIEF",
                    "flag": "MH",
                    "vessel_type": "Bulk Carrier",
                    "gross_tonnage": 45000,
                    "deadweight": 82000,
                    "year_built": 2005
                }
            ]
        })
    }

    fn mmsi_fixture() -> serde_json::Value {
        serde_json::json!({
            "mmsi": "123459999",
            "imo": "1111111",
            "name": "SANCTIONED VESSEL",
            "flag": "KP",
            "vessel_type": "Tanker",
            "owner": "Korea Shipping LLC",
            "AIS": {
                "lat": 35.0,
                "lng": 129.0,
                "timestamp": "2024-02-01T08:00:00Z"
            }
        })
    }

    #[test]
    fn maritime_parses_vessel_search_fixture_extracts_name_mmsi_flag() {
        let json = vessel_search_fixture();
        let vessels = parse_vessel_list(&json);

        assert_eq!(vessels.len(), 2);
        assert_eq!(vessels[0].mmsi, "123456789");
        assert_eq!(vessels[0].vessel_name, "NORTHERN STAR");
        assert_eq!(vessels[0].flag.as_deref(), Some("PA"));
        assert_eq!(vessels[0].vessel_type.as_deref(), Some("Yacht"));
        assert_eq!(vessels[0].imo_number.as_deref(), Some("1234567"));
    }

    #[test]
    fn maritime_extracts_owner_tonnage_year_built() {
        let json = vessel_search_fixture();
        let vessels = parse_vessel_list(&json);

        assert_eq!(vessels[0].owner.as_deref(), Some("Offshore Holdings Ltd"));
        assert_eq!(vessels[0].gross_tonnage, Some(450));
        assert_eq!(vessels[0].year_built, Some(2018));
        assert_eq!(vessels[1].deadweight, Some(82000));
    }

    #[test]
    fn maritime_parses_mmsi_lookup_with_nested_ais_position() {
        let json = mmsi_fixture();
        let vessel = parse_vessel_item(&json).unwrap();

        assert_eq!(vessel.mmsi, "123459999");
        assert_eq!(vessel.vessel_name, "SANCTIONED VESSEL");
        assert_eq!(vessel.flag.as_deref(), Some("KP"));
        assert!((vessel.last_lat.unwrap() - 35.0).abs() < f64::EPSILON);
        assert!((vessel.last_lon.unwrap() - 129.0).abs() < f64::EPSILON);
    }

    #[test]
    fn maritime_handles_empty_vessel_list() {
        let json = serde_json::json!({ "vessels": [] });
        let vessels = parse_vessel_list(&json);
        assert!(vessels.is_empty());

        let bare_empty = parse_vessel_list(&serde_json::json!([]));
        assert!(bare_empty.is_empty());
    }
}
