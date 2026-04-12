//! State environmental permit parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// A permit limit or threshold.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PermitLimit {
    /// Limit type or pollutant name.
    pub name: String,
    /// Limit value as text.
    pub value: String,
}

/// A historical compliance event tied to a permit.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PermitViolation {
    /// Event date.
    pub date: String,
    /// Event description.
    pub description: String,
}

/// A normalized environmental permit record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StateEnvPermit {
    /// Facility name.
    pub facility_name: String,
    /// Permit number.
    pub permit_number: Option<String>,
    /// Permit type.
    pub permit_type: Option<String>,
    /// Limits attached to the permit.
    pub permit_limits: Vec<PermitLimit>,
    /// Violation or enforcement history.
    pub violation_history: Vec<PermitViolation>,
}

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let from = haystack.find(start)? + start.len();
    let remainder = haystack.get(from..)?;
    let to = remainder.find(end)?;
    Some(remainder[..to].trim().to_string())
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

/// Parse a state permit fixture.
#[must_use]
pub fn parse_state_permit(document: &str) -> Option<StateEnvPermit> {
    let facility_name = extract_between(document, "data-facility-name=\"", "\"")?;
    let limit_names = collect_attr_values(document, "data-limit-name");
    let limit_values = collect_attr_values(document, "data-limit-value");
    let permit_limits = limit_names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            limit_values.get(index).map(|value| PermitLimit {
                name: name.clone(),
                value: value.clone(),
            })
        })
        .collect();

    let violation_dates = collect_attr_values(document, "data-violation-date");
    let violation_desc = collect_attr_values(document, "data-violation-desc");
    let violation_history = violation_dates
        .iter()
        .enumerate()
        .filter_map(|(index, date)| {
            violation_desc.get(index).map(|description| PermitViolation {
                date: date.clone(),
                description: description.clone(),
            })
        })
        .collect();

    Some(StateEnvPermit {
        facility_name,
        permit_number: extract_between(document, "data-permit-number=\"", "\""),
        permit_type: extract_between(document, "data-permit-type=\"", "\""),
        permit_limits,
        violation_history,
    })
}

/// Fetch a state environmental permit page.
/// 
/// # Errors
/// 
/// Returns `Err` if the request fails or the document cannot be written.
pub async fn fetch_state_permit(url: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError { status: status.as_u16(), body });
    }
    let body = resp.text().await?;
    let output_path = output_dir.join("state_env_permits.ndjson");
    let count = write_ndjson(&output_path, &[serde_json::json!({"url": url, "body": body})])?;
    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "state_env_permits".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn permit_fixture() -> &'static str {
        r#"
        <main data-facility-name="Delta Refining Terminal" data-permit-number="TX-AP-10422" data-permit-type="Air"></main>
        <div data-limit-name="NOx" data-limit-value="12.0 tons/year"></div>
        <div data-limit-name="SO2" data-limit-value="8.5 tons/year"></div>
        <div data-violation-date="2025-03-14" data-violation-desc="Exceeded quarterly VOC threshold"></div>
        "#
    }

    #[test]
    fn state_permits_fetcher_parses_air_water_permit_fixture() {
        let permit = parse_state_permit(permit_fixture()).unwrap();
        assert_eq!(permit.facility_name, "Delta Refining Terminal");
        assert_eq!(permit.permit_type.as_deref(), Some("Air"));
        assert_eq!(permit.permit_limits.len(), 2);
    }

    #[test]
    fn state_permits_fetcher_extracts_limits_and_violation_history() {
        let permit = parse_state_permit(permit_fixture()).unwrap();
        assert_eq!(permit.permit_limits[0].name, "NOx");
        assert_eq!(permit.permit_limits[1].value, "8.5 tons/year");
        assert_eq!(permit.violation_history.len(), 1);
        assert!(permit.violation_history[0]
            .description
            .contains("VOC threshold"));
    }
}