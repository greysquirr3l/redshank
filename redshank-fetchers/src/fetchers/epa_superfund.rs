//! EPA Superfund site parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SUPERFUND_BASE: &str = "https://enviro.epa.gov/facts/sems/search.html";

/// A responsible party tied to a Superfund site.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ResponsibleParty {
    /// Party name.
    pub name: String,
    /// Liability role or note.
    pub role: Option<String>,
}

/// A normalized EPA Superfund site record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SuperfundSite {
    /// Site name.
    pub site_name: String,
    /// EPA site identifier.
    pub epa_id: Option<String>,
    /// NPL status.
    pub npl_status: Option<String>,
    /// Hazard ranking score.
    pub hazard_ranking_score: Option<f64>,
    /// Cleanup cost estimate or reported expenditure.
    pub cleanup_cost_usd: Option<f64>,
    /// Responsible parties.
    pub responsible_parties: Vec<ResponsibleParty>,
}

/// Parse a Superfund site fixture.
#[must_use]
pub fn parse_superfund_site(json: &serde_json::Value) -> Option<SuperfundSite> {
    let site_name = json.get("site_name")?.as_str()?.to_string();
    let responsible_parties = json
        .get("prps")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(ResponsibleParty {
                name: entry.get("name")?.as_str()?.to_string(),
                role: entry
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string),
            })
        })
        .collect();

    Some(SuperfundSite {
        site_name,
        epa_id: json.get("epa_id").and_then(serde_json::Value::as_str).map(ToString::to_string),
        npl_status: json.get("npl_status").and_then(serde_json::Value::as_str).map(ToString::to_string),
        hazard_ranking_score: json.get("hazard_ranking_score").and_then(serde_json::Value::as_f64),
        cleanup_cost_usd: json.get("cleanup_cost_usd").and_then(serde_json::Value::as_f64),
        responsible_parties,
    })
}

/// Fetch Superfund site search results.
///
/// # Errors
///
/// Returns `Err` if the request fails.
pub async fn fetch_superfund_site(query: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(SUPERFUND_BASE).query(&[("query", query)]).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError { status: status.as_u16(), body });
    }

    let body = resp.text().await?;
    let records = vec![serde_json::json!({"query": query, "body": body})];
    let output_path = output_dir.join("epa_superfund.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "epa_superfund".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn epa_superfund_parses_npl_site_fixture_with_prp_list() {
        let json = serde_json::json!({
            "site_name": "Riverside Drum Disposal",
            "epa_id": "NJD980654321",
            "npl_status": "Final",
            "hazard_ranking_score": 54.22,
            "cleanup_cost_usd": 125000000.0,
            "prps": [
                {"name": "Acme Chemical Corp", "role": "Generator"},
                {"name": "Riverside Holdings LLC", "role": "Current owner"}
            ]
        });

        let site = parse_superfund_site(&json).unwrap();
        assert_eq!(site.site_name, "Riverside Drum Disposal");
        assert_eq!(site.responsible_parties.len(), 2);
        assert_eq!(site.npl_status.as_deref(), Some("Final"));
    }

    #[test]
    fn epa_superfund_extracts_cleanup_cost_and_responsible_parties() {
        let json = serde_json::json!({
            "site_name": "Riverside Drum Disposal",
            "cleanup_cost_usd": 125000000.0,
            "prps": [
                {"name": "Acme Chemical Corp", "role": "Generator"}
            ]
        });

        let site = parse_superfund_site(&json).unwrap();
        assert_eq!(site.cleanup_cost_usd, Some(125_000_000.0));
        assert_eq!(site.responsible_parties[0].name, "Acme Chemical Corp");
        assert_eq!(site.responsible_parties[0].role.as_deref(), Some("Generator"));
    }
}