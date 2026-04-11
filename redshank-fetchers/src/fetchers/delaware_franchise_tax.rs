//! Delaware Division of Corporations — entity status and franchise tax check.
//!
//! Source: Delaware ICIS entity search API
//! <https://icis.corp.delaware.gov/ecorp/entitysearch/NameSearch.aspx>
//!
//! Checks entity standing (Good Standing, Void, Forfeited, Cancelled, Inactive)
//! for cross-referencing corporate registrations.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const DELAWARE_API_BASE: &str = "https://icis.corp.delaware.gov/api";

/// Delaware entity status.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EntityStatus {
    /// Entity is current on franchise taxes and in good standing.
    GoodStanding,
    /// Entity has lost legal existence (3+ years of non-payment/non-filing).
    Void,
    /// Entity failed to maintain a registered agent.
    Forfeited,
    /// Entity was voluntarily dissolved.
    Cancelled,
    /// Entity merged into or converted to another entity.
    Inactive,
    /// Status not recognized or not available.
    Unknown(String),
}

impl EntityStatus {
    fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "GOOD STANDING" | "GOODSTANDING" => Self::GoodStanding,
            "VOID" => Self::Void,
            "FORFEITED" | "REVOKED" => Self::Forfeited,
            "CANCELLED" | "CANCELED" | "DISSOLVED" => Self::Cancelled,
            "INACTIVE" | "MERGED" | "CONVERTED" => Self::Inactive,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Return the canonical display string for the status.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::GoodStanding => "Good Standing",
            Self::Void => "Void",
            Self::Forfeited => "Forfeited",
            Self::Cancelled => "Cancelled",
            Self::Inactive => "Inactive",
            Self::Unknown(s) => s.as_str(),
        }
    }

    /// Return `true` if the entity is legally active (Good Standing).
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(self, Self::GoodStanding)
    }
}

/// A Delaware corporate entity record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DelawareEntity {
    /// Delaware file number.
    pub file_number: String,
    /// Entity name.
    pub entity_name: String,
    /// Entity type ("Corporation", "LLC", "LP", "LLP", etc.).
    pub entity_type: Option<String>,
    /// Residency ("Domestic" = incorporated in DE; "Foreign" = registered in DE).
    pub residency: Option<String>,
    /// Date of incorporation or formation (ISO 8601).
    pub formation_date: Option<String>,
    /// Current standing status.
    pub status: EntityStatus,
    /// Registered agent name.
    pub registered_agent: Option<String>,
    /// Registered agent address.
    pub registered_agent_address: Option<String>,
    /// Last annual report year filed.
    pub last_annual_report: Option<u32>,
    /// Franchise tax amount due (if any unpaid balance is shown).
    pub franchise_tax_due: Option<i64>,
}

/// Parse a Delaware ICIS JSON response into entity records.
///
/// Handles the search results array returned by the ICIS API.
#[must_use]
pub fn parse_delaware_entities(json: &serde_json::Value) -> Vec<DelawareEntity> {
    let arr = json
        .get("entities")
        .or_else(|| json.get("results"))
        .or_else(|| json.get("data"))
        .and_then(serde_json::Value::as_array)
        .or_else(|| json.as_array());

    arr.map(|items| items.iter().filter_map(parse_single_entity).collect())
        .unwrap_or_default()
}

fn parse_single_entity(item: &serde_json::Value) -> Option<DelawareEntity> {
    let str_field = |keys: &[&str]| -> Option<String> {
        keys.iter().find_map(|k| {
            item.get(*k)
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
    };

    let file_number = str_field(&["fileNumber", "file_number", "entityId", "fileNo"])?;
    let entity_name = str_field(&["entityName", "entity_name", "name"])?;

    let status_str =
        str_field(&["status", "entityStatus", "standing"]).unwrap_or_else(|| "Unknown".to_string());
    let status = EntityStatus::from_str(&status_str);

    let last_annual_report = item
        .get("lastAnnualReport")
        .or_else(|| item.get("lastFiled"))
        .and_then(serde_json::Value::as_u64)
        .map(|n| n as u32);

    let franchise_tax_due = item
        .get("franchiseTaxDue")
        .or_else(|| item.get("taxDue"))
        .and_then(serde_json::Value::as_i64);

    Some(DelawareEntity {
        file_number,
        entity_name,
        entity_type: str_field(&["entityType", "entity_type", "type"]),
        residency: str_field(&["residency", "domesticForeign"]),
        formation_date: str_field(&["formationDate", "incorporationDate", "dateFormed"]),
        status,
        registered_agent: str_field(&["registeredAgent", "agentName"]),
        registered_agent_address: str_field(&["agentAddress", "registeredAgentAddress"]),
        last_annual_report,
        franchise_tax_due,
    })
}

/// Fetch Delaware entity status by name or file number.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or returns a non-2xx status.
pub async fn fetch_delaware_entities(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    let resp = client
        .get(format!("{DELAWARE_API_BASE}/entity/search"))
        .query(&[("name", query)])
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
    let entities = parse_delaware_entities(&json);

    let serialized: Vec<serde_json::Value> = entities
        .iter()
        .filter_map(|e| serde_json::to_value(e).ok())
        .collect();

    let output_path = output_dir.join("delaware_entities.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "delaware_franchise_tax".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn delaware_fixture() -> serde_json::Value {
        serde_json::json!({
            "entities": [
                {
                    "fileNumber": "3456789",
                    "entityName": "WIDGET CORP",
                    "entityType": "Corporation",
                    "residency": "Domestic",
                    "formationDate": "2008-03-15",
                    "status": "Good Standing",
                    "registeredAgent": "The Corporation Trust Company",
                    "agentAddress": "1209 Orange St, Wilmington, DE 19801",
                    "lastAnnualReport": 2023,
                    "franchiseTaxDue": 0
                },
                {
                    "fileNumber": "7654321",
                    "entityName": "DEFUNCT LLC",
                    "entityType": "LLC",
                    "residency": "Domestic",
                    "formationDate": "2010-01-01",
                    "status": "Void",
                    "registeredAgent": "None",
                    "lastAnnualReport": 2019,
                    "franchiseTaxDue": 6400
                },
                {
                    "fileNumber": "9988776",
                    "entityName": "ABANDONED CORP INC",
                    "entityType": "Corporation",
                    "residency": "Domestic",
                    "formationDate": "2005-07-04",
                    "status": "Forfeited",
                    "lastAnnualReport": 2020
                }
            ]
        })
    }

    #[test]
    fn delaware_parses_entity_status_response_extracts_name_file_number() {
        let json = delaware_fixture();
        let entities = parse_delaware_entities(&json);

        assert_eq!(entities.len(), 3);
        assert_eq!(entities[0].file_number, "3456789");
        assert_eq!(entities[0].entity_name, "WIDGET CORP");
        assert_eq!(entities[0].entity_type.as_deref(), Some("Corporation"));
    }

    #[test]
    fn delaware_handles_good_standing_status() {
        let json = delaware_fixture();
        let entities = parse_delaware_entities(&json);

        assert_eq!(entities[0].status, EntityStatus::GoodStanding);
        assert!(entities[0].status.is_active());
        assert_eq!(
            entities[0].registered_agent.as_deref(),
            Some("The Corporation Trust Company")
        );
        assert_eq!(entities[0].last_annual_report, Some(2023));
    }

    #[test]
    fn delaware_handles_void_status_with_franchise_tax_due() {
        let json = delaware_fixture();
        let entities = parse_delaware_entities(&json);

        assert_eq!(entities[1].status, EntityStatus::Void);
        assert!(!entities[1].status.is_active());
        assert_eq!(entities[1].franchise_tax_due, Some(6400));
    }

    #[test]
    fn delaware_handles_forfeited_status() {
        let json = delaware_fixture();
        let entities = parse_delaware_entities(&json);

        assert_eq!(entities[2].status, EntityStatus::Forfeited);
        assert_eq!(entities[2].entity_name, "ABANDONED CORP INC");
    }
}
