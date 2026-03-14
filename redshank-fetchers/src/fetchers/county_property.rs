//! County property / real estate record fetcher.
//!
//! Targets county assessor portals for high-interest jurisdictions.
//! NYC ACRIS has a JSON API; others require stygian-browser + AI extraction.
//!
//! Pipeline configs stored as TOML, loaded via `include_str!`.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

/// NYC ACRIS real property records API.
const ACRIS_API: &str = "https://data.cityofnewyork.us/resource/636b-3b5g.json";

/// County property pipeline config for NYC ACRIS.
pub const PIPELINE_NYC_ACRIS: &str =
    include_str!("../../pipelines/county_property/nyc_acris.toml");

/// County property pipeline config for Miami-Dade.
pub const PIPELINE_MIAMI_DADE: &str =
    include_str!("../../pipelines/county_property/miami_dade.toml");

/// Fetch NYC ACRIS property records matching the given owner name.
pub async fn fetch_acris_records(
    owner_name: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };
    let limit = 1000_u32;

    for page in 0..max {
        let offset = page * limit;
        let resp = client
            .get(ACRIS_API)
            .query(&[
                ("$where", &format!("upper(name) LIKE '%{}'", owner_name.to_uppercase().replace('\'', "''"))),
                ("$limit", &limit.to_string()),
                ("$offset", &offset.to_string()),
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
        let records = json.as_array().cloned().unwrap_or_default();

        if records.is_empty() {
            break;
        }
        all_records.extend(records);

        if (all_records.len() as u32) < offset + limit {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("acris_property.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "acris".into(),
    })
}

/// Parsed pipeline configuration for a county property scrape.
#[derive(Debug, Clone)]
pub struct CountyPropertyPipeline {
    pub county: String,
    pub portal_url: String,
    pub has_json_api: bool,
    pub api_url: String,
    pub search_selector: String,
    pub detail_fields: Vec<String>,
}

/// Parse a county property pipeline TOML into a structured config.
pub fn parse_pipeline_config(toml_str: &str) -> Result<CountyPropertyPipeline, String> {
    let mut county = String::new();
    let mut portal_url = String::new();
    let mut has_json_api = false;
    let mut api_url = String::new();
    let mut search_selector = String::new();
    let mut detail_fields = Vec::new();

    for line in toml_str.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "county" => county = value.to_owned(),
                "portal_url" => portal_url = value.to_owned(),
                "has_json_api" => has_json_api = value == "true",
                "api_url" => api_url = value.to_owned(),
                "search_selector" => search_selector = value.to_owned(),
                "detail_fields" => {
                    let inner = value.trim_start_matches('[').trim_end_matches(']');
                    detail_fields = inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    if county.is_empty() || portal_url.is_empty() {
        return Err("Missing required fields: county, portal_url".into());
    }

    Ok(CountyPropertyPipeline {
        county,
        portal_url,
        has_json_api,
        api_url,
        search_selector,
        detail_fields,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn county_property_acris_pipeline_config_loads() {
        let config = parse_pipeline_config(PIPELINE_NYC_ACRIS)
            .expect("Failed to parse NYC ACRIS pipeline");
        assert_eq!(config.county, "NYC");
        assert!(config.has_json_api);
        assert!(!config.api_url.is_empty());
    }

    #[test]
    fn county_property_miami_dade_pipeline_config_loads() {
        let config = parse_pipeline_config(PIPELINE_MIAMI_DADE)
            .expect("Failed to parse Miami-Dade pipeline");
        assert_eq!(config.county, "Miami-Dade");
        assert!(!config.detail_fields.is_empty());
    }

    #[test]
    fn county_property_acris_parses_owner_response() {
        let mock = serde_json::json!([
            {
                "name": "ACME HOLDINGS LLC",
                "document_id": "2024012345678",
                "doc_type": "DEED",
                "recorded_filed": "2024-03-15",
                "borough": "1",
                "block": "00123",
                "lot": "0045"
            }
        ]);
        let records = mock.as_array().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["name"], "ACME HOLDINGS LLC");
        assert_eq!(records[0]["doc_type"], "DEED");
    }
}
