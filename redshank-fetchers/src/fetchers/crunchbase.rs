//! Crunchbase startup and investor intelligence fetcher.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client_with_key, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.crunchbase.com/api/v4";

/// A Crunchbase person search result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CrunchbasePerson {
    /// Person display name.
    pub name: String,
    /// Current title.
    pub title: Option<String>,
    /// Primary location.
    pub location: Option<String>,
    /// Crunchbase permalink.
    pub permalink: Option<String>,
    /// Current primary organization.
    pub primary_organization: Option<String>,
}

/// A Crunchbase funding round.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CrunchbaseFundingRound {
    /// Series name, such as `Series A`.
    pub series: Option<String>,
    /// Announcement date.
    pub announced_on: Option<String>,
    /// Amount raised in USD.
    pub money_raised_usd: Option<u64>,
    /// Lead investors for the round.
    pub lead_investors: Vec<String>,
}

/// A Crunchbase organization entity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CrunchbaseOrganization {
    /// Organization name.
    pub name: String,
    /// Organization permalink.
    pub permalink: Option<String>,
    /// Short description.
    pub short_description: Option<String>,
    /// Founded date.
    pub founded_on: Option<String>,
    /// Headquarters string.
    pub headquarters: Option<String>,
    /// Category groups.
    pub categories: Vec<String>,
    /// Employee range enum.
    pub num_employees_enum: Option<String>,
    /// Total funding in USD.
    pub total_funding_usd: Option<u64>,
    /// Known investors.
    pub investors: Vec<String>,
    /// Board members and advisors.
    pub board_members: Vec<String>,
    /// Funding rounds.
    pub funding_rounds: Vec<CrunchbaseFundingRound>,
}

fn optional_string(json: &serde_json::Value, key: &str) -> Option<String> {
    json.get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn optional_u64(json: &serde_json::Value, key: &str) -> Option<u64> {
    json.get(key).and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
    })
}

/// Parse a Crunchbase person search response.
#[must_use]
pub fn parse_person_search(json: &serde_json::Value) -> Vec<CrunchbasePerson> {
    json.get("entities")
        .or_else(|| json.get("results"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let properties = item.get("properties").unwrap_or(item);
                    let name = optional_string(properties, "name")?;
                    let location = item
                        .get("cards")
                        .and_then(|cards| cards.get("fields"))
                        .and_then(|fields| fields.get("location_identifiers"))
                        .and_then(serde_json::Value::as_array)
                        .and_then(|locations| locations.first())
                        .and_then(|location| location.get("value"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string);

                    Some(CrunchbasePerson {
                        name,
                        title: optional_string(properties, "title"),
                        location,
                        permalink: optional_string(properties, "permalink"),
                        primary_organization: optional_string(
                            properties,
                            "primary_organization_name",
                        ),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a Crunchbase organization entity response.
#[must_use]
pub fn parse_organization(json: &serde_json::Value) -> Option<CrunchbaseOrganization> {
    let properties = json.get("properties").unwrap_or(json);
    let cards = json.get("cards").unwrap_or(&serde_json::Value::Null);
    let name = optional_string(properties, "name")?;

    let categories = cards
        .get("category_groups")
        .and_then(|group| group.get("category_groups"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("value").and_then(serde_json::Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let investors = cards
        .get("investors")
        .and_then(|investors| investors.get("items"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| optional_string(item, "name"))
                .collect()
        })
        .unwrap_or_default();

    let board_members = cards
        .get("board_members_and_advisors")
        .and_then(|board| board.get("items"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| optional_string(item, "name"))
                .collect()
        })
        .unwrap_or_default();

    let funding_rounds = cards
        .get("funding_rounds")
        .and_then(|rounds| rounds.get("items"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let lead_investors = item
                        .get("lead_investors")
                        .and_then(serde_json::Value::as_array)
                        .map(|investors| {
                            investors
                                .iter()
                                .filter_map(|investor| optional_string(investor, "name"))
                                .collect()
                        })
                        .unwrap_or_default();

                    CrunchbaseFundingRound {
                        series: optional_string(item, "investment_type"),
                        announced_on: optional_string(item, "announced_on"),
                        money_raised_usd: optional_u64(item, "money_raised_usd"),
                        lead_investors,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let headquarters = ["city_name", "region_name", "country_code"]
        .iter()
        .filter_map(|key| optional_string(properties, key))
        .collect::<Vec<_>>()
        .join(", ");

    Some(CrunchbaseOrganization {
        name,
        permalink: optional_string(properties, "permalink"),
        short_description: optional_string(properties, "short_description"),
        founded_on: optional_string(properties, "founded_on"),
        headquarters: if headquarters.is_empty() {
            None
        } else {
            Some(headquarters)
        },
        categories,
        num_employees_enum: optional_string(properties, "num_employees_enum"),
        total_funding_usd: optional_u64(properties, "funding_total_usd"),
        investors,
        board_members,
        funding_rounds,
    })
}

/// Fetch a Crunchbase organization entity.
///
/// # Errors
///
/// Returns `Err` if the request fails, the server returns a non-success status,
/// or the response cannot be parsed.
pub async fn fetch_organization(
    permalink: &str,
    api_key: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client_with_key("X-cb-user-key", api_key)?;
    let resp = client
        .get(format!("{API_BASE}/entities/organizations/{permalink}"))
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
    let record = parse_organization(&json)
        .ok_or_else(|| FetchError::Parse("could not parse Crunchbase organization".to_string()))?;
    let records =
        vec![serde_json::to_value(record).map_err(|err| FetchError::Parse(err.to_string()))?];
    let output_path = output_dir.join("crunchbase.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "crunchbase".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn person_search_fixture() -> serde_json::Value {
        serde_json::json!({
            "entities": [
                {
                    "properties": {
                        "name": "Jane Founder",
                        "title": "Founder & CEO",
                        "primary_organization_name": "Grey Heron Labs",
                        "permalink": "jane-founder"
                    },
                    "cards": {
                        "fields": {
                            "location_identifiers": [{"value": "London"}]
                        }
                    }
                }
            ]
        })
    }

    fn organization_fixture() -> serde_json::Value {
        serde_json::json!({
            "properties": {
                "name": "Grey Heron Labs",
                "permalink": "grey-heron-labs",
                "short_description": "Investigative data tooling",
                "founded_on": "2019-02-01",
                "city_name": "London",
                "region_name": "England",
                "country_code": "GB",
                "num_employees_enum": "11-50",
                "funding_total_usd": 12_500_000
            },
            "cards": {
                "funding_rounds": {
                    "items": [
                        {
                            "investment_type": "Series A",
                            "announced_on": "2023-06-01",
                            "money_raised_usd": 9_000_000,
                            "lead_investors": [
                                {"name": "North Sea Ventures"},
                                {"name": "Signal Capital"}
                            ]
                        }
                    ]
                },
                "investors": {
                    "items": [
                        {"name": "North Sea Ventures"},
                        {"name": "Signal Capital"}
                    ]
                },
                "board_members_and_advisors": {
                    "items": [
                        {"name": "Jane Founder"},
                        {"name": "Omar Investor"}
                    ]
                },
                "category_groups": {
                    "category_groups": [
                        {"value": "Data and Analytics"},
                        {"value": "Security"}
                    ]
                }
            }
        })
    }

    #[test]
    fn crunchbase_parses_person_search_fixture() {
        let people = parse_person_search(&person_search_fixture());

        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "Jane Founder");
        assert_eq!(people[0].title.as_deref(), Some("Founder & CEO"));
        assert_eq!(people[0].location.as_deref(), Some("London"));
    }

    #[test]
    fn crunchbase_parses_organization_fixture_with_funding_rounds() {
        let org = parse_organization(&organization_fixture()).unwrap();

        assert_eq!(org.name, "Grey Heron Labs");
        assert_eq!(org.total_funding_usd, Some(12_500_000));
        assert_eq!(org.funding_rounds.len(), 1);
        assert_eq!(org.funding_rounds[0].series.as_deref(), Some("Series A"));
        assert_eq!(org.funding_rounds[0].money_raised_usd, Some(9_000_000));
    }

    #[test]
    fn crunchbase_extracts_board_members_and_investors() {
        let org = parse_organization(&organization_fixture()).unwrap();

        assert!(org.board_members.contains(&"Jane Founder".to_string()));
        assert!(org.investors.contains(&"North Sea Ventures".to_string()));
        assert!(
            org.funding_rounds[0]
                .lead_investors
                .contains(&"Signal Capital".to_string())
        );
    }
}
