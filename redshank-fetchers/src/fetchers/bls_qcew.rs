//! Bureau of Labor Statistics QCEW parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.bls.gov/publicAPI/v2/timeseries/data/";

/// A normalized QCEW area/industry record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct QcewRecord {
    pub area_fips: String,
    pub area_title: String,
    pub industry_code: String,
    pub industry_title: String,
    pub ownership_code: Option<String>,
    pub annual_avg_emplvl: Option<f64>,
    pub total_wages: Option<f64>,
    pub avg_weekly_wage: Option<f64>,
    pub establishments: Option<f64>,
    pub year: Option<u32>,
}

fn opt_string(record: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| record.get(*key).and_then(serde_json::Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn opt_f64(record: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        record.get(*key).and_then(|value| {
            value
                .as_f64()
                .or_else(|| {
                    #[allow(clippy::cast_precision_loss)]
                    value.as_i64().map(|number| number as f64)
                })
                .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        })
    })
}

/// Parse QCEW fixture or API-adjacent JSON into normalized records.
#[must_use]
pub fn parse_qcew_records(json: &serde_json::Value) -> Vec<QcewRecord> {
    json.get("Results")
        .or_else(|| json.get("results"))
        .or_else(|| json.get("data"))
        .and_then(serde_json::Value::as_array)
        .map(|records| {
            records
                .iter()
                .filter_map(|record| {
                    Some(QcewRecord {
                        area_fips: opt_string(record, &["area_fips", "area_fips_code"])?,
                        area_title: opt_string(record, &["area_title"])?,
                        industry_code: opt_string(record, &["industry_code", "industry"])?,
                        industry_title: opt_string(record, &["industry_title"])?,
                        ownership_code: opt_string(record, &["ownership_code"]),
                        annual_avg_emplvl: opt_f64(record, &["annual_avg_emplvl", "avg_emplvl"]),
                        total_wages: opt_f64(record, &["total_annual_wages", "total_wages"]),
                        avg_weekly_wage: opt_f64(record, &["avg_wkly_wage", "avg_weekly_wage"]),
                        establishments: opt_f64(record, &["annual_avg_estabs", "establishments"]),
                        year: opt_string(record, &["year"])
                            .and_then(|year| year.parse::<u32>().ok()),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch a BLS timeseries response for one or more series ids.
///
/// # Errors
///
/// Returns `Err` if the request fails or the response status is non-success.
pub async fn fetch_series(
    series_ids: &[String],
    api_key: Option<&str>,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut payload = serde_json::json!({"seriesid": series_ids});
    if let Some(key) = api_key
        && let Some(obj) = payload.as_object_mut()
    {
        obj.insert(
            "registrationkey".to_string(),
            serde_json::Value::String(key.to_string()),
        );
    }

    let resp = client.post(API_BASE).json(&payload).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let output_path = output_dir.join("bls_qcew.ndjson");
    let count = write_ndjson(&output_path, &[json])?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "bls-qcew".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn qcew_fixture() -> serde_json::Value {
        serde_json::json!({
            "Results": [
                {
                    "area_fips": "36061",
                    "area_title": "New York County, NY",
                    "industry_code": "541110",
                    "industry_title": "Offices of Lawyers",
                    "ownership_code": "5",
                    "annual_avg_emplvl": "15423",
                    "total_annual_wages": "2567890000",
                    "avg_wkly_wage": "3201",
                    "annual_avg_estabs": "2123",
                    "year": "2024"
                }
            ]
        })
    }

    #[test]
    fn bls_qcew_parses_establishment_level_fixture() {
        let records = parse_qcew_records(&qcew_fixture());

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].area_fips, "36061");
        assert_eq!(records[0].industry_code, "541110");
    }

    #[test]
    fn bls_qcew_extracts_employer_size_industry_and_wages_by_area() {
        let records = parse_qcew_records(&qcew_fixture());

        assert_eq!(records[0].area_title, "New York County, NY");
        assert_eq!(records[0].industry_title, "Offices of Lawyers");
        assert_eq!(records[0].annual_avg_emplvl, Some(15_423.0));
        assert_eq!(records[0].avg_weekly_wage, Some(3201.0));
        assert_eq!(records[0].establishments, Some(2123.0));
    }
}
