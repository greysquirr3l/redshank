//! Voluntary carbon registry parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const VERRA_BASE: &str = "https://registry.verra.org/uiapi/asset/crediting-period";

/// A carbon credit retirement event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RetirementRecord {
    /// Retirement date.
    pub date: String,
    /// Retiring account or organization.
    pub retired_by: String,
    /// Quantity retired.
    pub quantity: u64,
}

/// A normalized carbon project record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CarbonProject {
    /// Project name.
    pub project_name: String,
    /// Project identifier.
    pub project_id: String,
    /// Verification status.
    pub verification_status: Option<String>,
    /// Developer or proponent.
    pub developer: Option<String>,
    /// Retirement records.
    pub retirements: Vec<RetirementRecord>,
}

/// Parse a carbon registry project fixture.
#[must_use]
pub fn parse_carbon_project(json: &serde_json::Value) -> Option<CarbonProject> {
    let project_name = json.get("project_name")?.as_str()?.to_string();
    let project_id = json.get("project_id")?.as_str()?.to_string();
    let retirements = json
        .get("retirements")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(RetirementRecord {
                date: entry.get("date")?.as_str()?.to_string(),
                retired_by: entry.get("retired_by")?.as_str()?.to_string(),
                quantity: entry.get("quantity")?.as_u64()?,
            })
        })
        .collect();

    Some(CarbonProject {
        project_name,
        project_id,
        verification_status: json.get("verification_status").and_then(serde_json::Value::as_str).map(ToString::to_string),
        developer: json.get("developer").and_then(serde_json::Value::as_str).map(ToString::to_string),
        retirements,
    })
}

/// Fetch a carbon registry project JSON payload.
/// 
/// # Errors
/// 
/// Returns `Err` if the request fails.
pub async fn fetch_carbon_project(project_id: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(format!("{VERRA_BASE}/{project_id}")).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError { status: status.as_u16(), body });
    }
    let json: serde_json::Value = resp.json().await?;
    let output_path = output_dir.join("carbon_registries.ndjson");
    let count = write_ndjson(&output_path, &[json])?;
    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "carbon_registries".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn carbon_registry_fetcher_parses_offset_project_fixture() {
        let json = serde_json::json!({
            "project_name": "Delta Mangrove Restoration",
            "project_id": "VCS-1942",
            "verification_status": "Verified",
            "developer": "Blue Carbon Partners",
            "retirements": []
        });

        let project = parse_carbon_project(&json).unwrap();
        assert_eq!(project.project_name, "Delta Mangrove Restoration");
        assert_eq!(project.project_id, "VCS-1942");
        assert_eq!(project.verification_status.as_deref(), Some("Verified"));
    }

    #[test]
    fn carbon_registry_fetcher_extracts_retirement_records_and_verification_status() {
        let json = serde_json::json!({
            "project_name": "Delta Mangrove Restoration",
            "project_id": "VCS-1942",
            "verification_status": "Verified",
            "retirements": [
                {"date": "2025-11-01", "retired_by": "Acme Manufacturing", "quantity": 12500}
            ]
        });

        let project = parse_carbon_project(&json).unwrap();
        assert_eq!(project.retirements.len(), 1);
        assert_eq!(project.retirements[0].retired_by, "Acme Manufacturing");
        assert_eq!(project.retirements[0].quantity, 12_500);
    }
}