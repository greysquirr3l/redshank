//! SEC climate disclosure parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SEC_SUBMISSIONS_BASE: &str = "https://data.sec.gov/submissions";

/// A normalized SEC climate disclosure record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SecClimateDisclosure {
    /// SEC CIK identifier.
    pub cik: String,
    /// Whether the filing appears aligned to `TCFD` terminology.
    pub tcfd_aligned: bool,
    /// Scope 1 emissions in metric tons `CO2e`.
    pub scope1_emissions_mtco2e: Option<f64>,
    /// Scope 2 emissions in metric tons `CO2e`.
    pub scope2_emissions_mtco2e: Option<f64>,
    /// Scope 3 emissions in metric tons `CO2e`.
    pub scope3_emissions_mtco2e: Option<f64>,
    /// Physical climate risks described in the filing.
    pub physical_risks: Vec<String>,
    /// Transition climate risks described in the filing.
    pub transition_risks: Vec<String>,
    /// Governance narrative summary.
    pub climate_governance: Option<String>,
}

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let from = haystack.find(start)? + start.len();
    let remainder = haystack.get(from..)?;
    let to = remainder.find(end)?;
    Some(remainder[..to].trim().to_string())
}

fn extract_metric(document: &str, field: &str) -> Option<f64> {
    extract_between(document, &format!("{field}=\""), "\"")
        .and_then(|value| value.replace(',', "").parse::<f64>().ok())
}

fn collect_attr_values(html: &str, attr: &str) -> Vec<String> {
    let marker = format!("{attr}=\"");
    let mut values = Vec::new();
    let mut remainder = html;

    while let Some(idx) = remainder.find(&marker) {
        let after = &remainder[idx + marker.len()..];
        let Some(end_idx) = after.find('"') else {
            break;
        };
        let value = after[..end_idx].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        remainder = &after[end_idx + 1..];
    }

    values
}

/// Parse a 10-K climate disclosure fixture.
#[must_use]
pub fn parse_climate_disclosure(cik: &str, document: &str) -> Option<SecClimateDisclosure> {
    Some(SecClimateDisclosure {
        cik: cik.to_string(),
        tcfd_aligned: document.contains("TCFD")
            || document.contains("Task Force on Climate-related Financial Disclosures"),
        scope1_emissions_mtco2e: extract_metric(document, "data-scope1"),
        scope2_emissions_mtco2e: extract_metric(document, "data-scope2"),
        scope3_emissions_mtco2e: extract_metric(document, "data-scope3"),
        physical_risks: collect_attr_values(document, "data-physical-risk"),
        transition_risks: collect_attr_values(document, "data-transition-risk"),
        climate_governance: extract_between(document, "data-climate-governance=\"", "\""),
    })
}

/// Fetch SEC submissions JSON for a climate disclosure source document.
///
/// # Errors
///
/// Returns `Err` if the submissions request fails.
pub async fn fetch_sec_climate(cik: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get(format!("{SEC_SUBMISSIONS_BASE}/{cik}.json"))
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
    let output_path = output_dir.join("sec_climate.ndjson");
    let count = write_ndjson(&output_path, &[json])?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "sec_climate".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn climate_fixture() -> &'static str {
        r#"
        <section data-scope1="125000" data-scope2="78000" data-scope3="540000" data-climate-governance="Board sustainability committee reviews climate metrics quarterly.">
            <p>Our disclosure aligns with TCFD recommendations and SEC climate rules.</p>
            <div data-physical-risk="Hurricane disruption to Gulf Coast facilities"></div>
            <div data-physical-risk="Wildfire exposure near transmission lines"></div>
            <div data-transition-risk="Carbon pricing could increase input costs"></div>
        </section>
        "#
    }

    #[test]
    fn sec_climate_parses_tcfd_disclosure_fixture_from_10k() {
        let disclosure = parse_climate_disclosure("CIK0000123456", climate_fixture()).unwrap();
        assert!(disclosure.tcfd_aligned);
        assert_eq!(disclosure.physical_risks.len(), 2);
        assert!(
            disclosure
                .climate_governance
                .as_deref()
                .unwrap()
                .contains("Board sustainability committee")
        );
    }

    #[test]
    fn sec_climate_extracts_scope_emissions_figures() {
        let disclosure = parse_climate_disclosure("CIK0000123456", climate_fixture()).unwrap();
        assert_eq!(disclosure.scope1_emissions_mtco2e, Some(125_000.0));
        assert_eq!(disclosure.scope2_emissions_mtco2e, Some(78_000.0));
        assert_eq!(disclosure.scope3_emissions_mtco2e, Some(540_000.0));
    }
}
