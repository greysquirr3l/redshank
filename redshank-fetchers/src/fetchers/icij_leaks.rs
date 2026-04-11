//! ICIJ Offshore Leaks — offshore entity, officer, intermediary, and address data.
//!
//! Source: <https://offshoreleaks.icij.org/>.
//! This module supports both legacy CSV parsing and structured API access.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const _BULK_URL: &str =
    "https://offshoreleaks-data.icij.org/offshoreleaks/csv/full-oldb.LATEST.zip";
const SEARCH_URL: &str = "https://offshoreleaks.icij.org/search";
const API_DELAY_MS: u64 = 250;

/// A relationship between offshore leaks nodes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OffshoreRelationship {
    /// Relationship type, such as `officer_of` or `registered_address`.
    pub relation_type: String,
    /// Direction relative to the focal node, such as `incoming` or `outgoing`.
    pub direction: String,
    /// Related node identifier.
    pub node_id: String,
    /// Related node name.
    pub name: String,
    /// Related node type.
    pub node_type: String,
}

/// A structured Offshore Leaks node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OffshoreNode {
    /// ICIJ node identifier.
    pub node_id: String,
    /// Display name.
    pub name: String,
    /// Node type, such as `Entity`, `Officer`, `Intermediary`, or `Address`.
    pub node_type: String,
    /// Incorporation jurisdiction.
    pub jurisdiction: Option<String>,
    /// Incorporation date.
    pub incorporation_date: Option<String>,
    /// Inactivation date.
    pub inactivation_date: Option<String>,
    /// Status, if present.
    pub status: Option<String>,
    /// Leak source identifier or label.
    pub source: Option<String>,
    /// Related nodes from the details API.
    pub relationships: Vec<OffshoreRelationship>,
}

/// A parsed search page from the Offshore Leaks API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OffshoreSearchPage {
    /// Parsed nodes for the current page.
    pub results: Vec<OffshoreNode>,
    /// Current page number.
    pub page: u32,
    /// Total pages, when available.
    pub total_pages: Option<u32>,
    /// Whether another page is available.
    pub has_next_page: bool,
}

fn str_field<'a>(json: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| json.get(*key).and_then(serde_json::Value::as_str))
}

fn optional_string_field(json: &serde_json::Value, keys: &[&str]) -> Option<String> {
    str_field(json, keys)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn parse_relationship(json: &serde_json::Value) -> Option<OffshoreRelationship> {
    let node_id = str_field(json, &["node_id", "id", "related_node_id"])?.to_string();

    Some(OffshoreRelationship {
        relation_type: str_field(json, &["type", "relationship_type"])
            .unwrap_or_default()
            .to_string(),
        direction: str_field(json, &["direction"])
            .unwrap_or("unknown")
            .to_string(),
        node_id,
        name: str_field(json, &["name", "label"])
            .unwrap_or_default()
            .to_string(),
        node_type: str_field(json, &["node_type", "type_name", "kind"])
            .unwrap_or_default()
            .to_string(),
    })
}

fn parse_node(json: &serde_json::Value) -> Option<OffshoreNode> {
    let node_id = str_field(json, &["node_id", "id"])?;
    let name = str_field(json, &["name", "label"])?;
    let node_type = str_field(json, &["node_type", "type", "kind"])?;

    let relationships = json
        .get("relationships")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().filter_map(parse_relationship).collect())
        .unwrap_or_default();

    Some(OffshoreNode {
        node_id: node_id.to_string(),
        name: name.to_string(),
        node_type: node_type.to_string(),
        jurisdiction: optional_string_field(json, &["jurisdiction", "jurisdiction_description"]),
        incorporation_date: optional_string_field(json, &["incorporation_date"]),
        inactivation_date: optional_string_field(json, &["inactivation_date"]),
        status: optional_string_field(json, &["status"]),
        source: optional_string_field(json, &["source", "source_id", "dataset"]),
        relationships,
    })
}

fn parse_u32_field(json: &serde_json::Value, keys: &[&str]) -> Option<u32> {
    keys.iter().find_map(|key| {
        json.get(*key).and_then(|value| {
            value
                .as_u64()
                .and_then(|number| u32::try_from(number).ok())
                .or_else(|| value.as_str().and_then(|text| text.parse::<u32>().ok()))
        })
    })
}

/// Parse an Offshore Leaks search response.
#[must_use]
pub fn parse_search_page(json: &serde_json::Value) -> OffshoreSearchPage {
    let results_value = json
        .get("results")
        .or_else(|| json.get("nodes"))
        .or_else(|| json.get("data"));

    let results = results_value
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().filter_map(parse_node).collect())
        .or_else(|| {
            json.as_array()
                .map(|items| items.iter().filter_map(parse_node).collect())
        })
        .unwrap_or_default();

    let pagination = json.get("pagination").unwrap_or(json);
    let page = parse_u32_field(pagination, &["page", "current_page"]).unwrap_or(1);
    let total_pages = parse_u32_field(pagination, &["pages", "total_pages", "last_page"]);
    let next_page = parse_u32_field(pagination, &["next_page", "next"]);
    let has_next_page = next_page.is_some()
        || total_pages.is_some_and(|pages| page < pages)
        || json
            .get("has_next_page")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

    OffshoreSearchPage {
        results,
        page,
        total_pages,
        has_next_page,
    }
}

/// Parse an Offshore Leaks node details response.
#[must_use]
pub fn parse_node_details(json: &serde_json::Value) -> Option<OffshoreNode> {
    json.get("node")
        .and_then(parse_node)
        .or_else(|| parse_node(json))
}

/// Extract officers linked to an entity.
#[must_use]
pub fn extract_officers(node: &OffshoreNode) -> Vec<OffshoreRelationship> {
    node.relationships
        .iter()
        .filter(|relationship| {
            relationship.relation_type == "officer_of"
                || relationship.node_type.eq_ignore_ascii_case("officer")
        })
        .cloned()
        .collect()
}

/// Extract intermediaries linked to an entity.
#[must_use]
pub fn extract_intermediaries(node: &OffshoreNode) -> Vec<OffshoreRelationship> {
    node.relationships
        .iter()
        .filter(|relationship| {
            relationship.relation_type == "intermediary_of"
                || relationship.node_type.eq_ignore_ascii_case("intermediary")
        })
        .cloned()
        .collect()
}

/// Extract registered addresses linked to an entity.
#[must_use]
pub fn extract_registered_addresses(node: &OffshoreNode) -> Vec<OffshoreRelationship> {
    node.relationships
        .iter()
        .filter(|relationship| {
            relationship.relation_type == "registered_address"
                || relationship.node_type.eq_ignore_ascii_case("address")
        })
        .cloned()
        .collect()
}

/// Collect leak sources represented in a search page.
#[must_use]
pub fn collect_sources(page: &OffshoreSearchPage) -> BTreeSet<String> {
    page.results
        .iter()
        .filter_map(|node| node.source.clone())
        .collect()
}

/// Traverse an entity network by following officers to their other linked entities.
#[must_use]
pub fn traverse_entity_officer_graph(
    entity: &OffshoreNode,
    related_nodes: &BTreeMap<String, OffshoreNode>,
) -> Vec<String> {
    let mut visited = BTreeSet::new();
    visited.insert(entity.node_id.clone());

    for officer in extract_officers(entity) {
        visited.insert(officer.node_id.clone());

        if let Some(officer_node) = related_nodes.get(&officer.node_id) {
            for relationship in &officer_node.relationships {
                if relationship.relation_type == "officer_of"
                    || relationship.node_type.eq_ignore_ascii_case("entity")
                {
                    visited.insert(relationship.node_id.clone());
                }
            }
        }
    }

    visited.into_iter().collect()
}

/// Parse a CSV line into entity fields (simplified parser for ICIJ nodes).
#[must_use]
pub fn parse_entity_csv_line(line: &str) -> Option<serde_json::Value> {
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 3 {
        return None;
    }
    Some(serde_json::json!({
        "node_id": fields.first().copied().unwrap_or("").trim_matches('"'),
        "name": fields.get(1).copied().unwrap_or("").trim_matches('"'),
        "jurisdiction": fields.get(2).copied().unwrap_or("").trim_matches('"'),
        "source": "icij-leaks"
    }))
}

/// Fetch ICIJ offshore leaks entity search results across all pages.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_entities(query: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut page = 1_u32;
    let mut records = Vec::new();

    loop {
        let resp = client
            .get(SEARCH_URL)
            .query(&[
                ("q", query),
                ("type", "Entity"),
                ("page", &page.to_string()),
            ])
            .header("accept", "application/json")
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
        let parsed = parse_search_page(&json);
        let mut page_records: Vec<serde_json::Value> = parsed
            .results
            .into_iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|err| FetchError::Parse(err.to_string()))?;
        records.append(&mut page_records);

        if !parsed.has_next_page {
            break;
        }

        page += 1;
        rate_limit_delay(API_DELAY_MS).await;
    }

    let output_path = output_dir.join("icij_leaks.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "icij-leaks".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    fn search_fixture() -> serde_json::Value {
        serde_json::json!({
            "results": [
                {
                    "node_id": "12345678",
                    "name": "EXAMPLE HOLDINGS LTD",
                    "node_type": "Entity",
                    "jurisdiction": "British Virgin Islands",
                    "source_id": "Panama Papers"
                },
                {
                    "node_id": "87654321",
                    "name": "Jane Doe",
                    "node_type": "Officer",
                    "jurisdiction": "Panama",
                    "source_id": "Paradise Papers"
                },
                {
                    "node_id": "55555555",
                    "name": "ALPHA TRUST",
                    "node_type": "Entity",
                    "jurisdiction": "Bahamas",
                    "source_id": "Pandora Papers"
                }
            ],
            "pagination": {
                "page": 1,
                "pages": 3,
                "next_page": 2
            }
        })
    }

    fn detail_fixture() -> serde_json::Value {
        serde_json::json!({
            "node": {
                "node_id": "12345678",
                "name": "EXAMPLE HOLDINGS LTD",
                "node_type": "Entity",
                "jurisdiction": "British Virgin Islands",
                "incorporation_date": "2005-03-15",
                "inactivation_date": "2016-04-01",
                "status": "Inactive",
                "source": "Panama Papers",
                "relationships": [
                    {
                        "type": "officer_of",
                        "direction": "incoming",
                        "node_id": "87654321",
                        "name": "John Doe",
                        "node_type": "Officer"
                    },
                    {
                        "type": "intermediary_of",
                        "direction": "incoming",
                        "node_id": "11111111",
                        "name": "Mossack Fonseca",
                        "node_type": "Intermediary"
                    },
                    {
                        "type": "registered_address",
                        "direction": "outgoing",
                        "node_id": "22222222",
                        "name": "P.O. Box 123, Road Town",
                        "node_type": "Address"
                    }
                ]
            }
        })
    }

    fn officer_detail_fixture() -> OffshoreNode {
        parse_node_details(&serde_json::json!({
            "node": {
                "node_id": "87654321",
                "name": "John Doe",
                "node_type": "Officer",
                "source": "Panama Papers",
                "relationships": [
                    {
                        "type": "officer_of",
                        "direction": "outgoing",
                        "node_id": "12345678",
                        "name": "EXAMPLE HOLDINGS LTD",
                        "node_type": "Entity"
                    },
                    {
                        "type": "officer_of",
                        "direction": "outgoing",
                        "node_id": "99999999",
                        "name": "SECOND SHELL CORP",
                        "node_type": "Entity"
                    }
                ]
            }
        }))
        .unwrap()
    }

    #[test]
    fn icij_csv_parser_extracts_node_fields() {
        let line = r#""10000001","Acme Offshore Ltd","BVI""#;
        let record = parse_entity_csv_line(line).unwrap();
        assert_eq!(record["node_id"], "10000001");
        assert_eq!(record["name"], "Acme Offshore Ltd");
        assert_eq!(record["jurisdiction"], "BVI");
    }

    #[test]
    fn icij_search_parser_returns_structured_results() {
        let page = parse_search_page(&search_fixture());

        assert_eq!(page.results.len(), 3);
        assert_eq!(page.results[0].node_id, "12345678");
        assert_eq!(page.results[0].name, "EXAMPLE HOLDINGS LTD");
        assert_eq!(page.results[0].node_type, "Entity");
    }

    #[test]
    fn icij_detail_parser_extracts_entity_relationships() {
        let node = parse_node_details(&detail_fixture()).unwrap();

        assert_eq!(node.node_id, "12345678");
        assert_eq!(node.status.as_deref(), Some("Inactive"));
        assert_eq!(node.relationships.len(), 3);
    }

    #[test]
    fn icij_extracts_officers_linked_to_entity() {
        let node = parse_node_details(&detail_fixture()).unwrap();
        let officers = extract_officers(&node);

        assert_eq!(officers.len(), 1);
        assert_eq!(officers[0].name, "John Doe");
        assert_eq!(officers[0].node_type, "Officer");
    }

    #[test]
    fn icij_extracts_intermediary_relationships() {
        let node = parse_node_details(&detail_fixture()).unwrap();
        let intermediaries = extract_intermediaries(&node);

        assert_eq!(intermediaries.len(), 1);
        assert_eq!(intermediaries[0].name, "Mossack Fonseca");
    }

    #[test]
    fn icij_extracts_registered_addresses() {
        let node = parse_node_details(&detail_fixture()).unwrap();
        let addresses = extract_registered_addresses(&node);

        assert_eq!(addresses.len(), 1);
        assert!(addresses[0].name.contains("Road Town"));
    }

    #[test]
    fn icij_handles_multiple_leak_sources() {
        let page = parse_search_page(&search_fixture());
        let sources = collect_sources(&page);

        assert!(sources.contains("Panama Papers"));
        assert!(sources.contains("Paradise Papers"));
        assert!(sources.contains("Pandora Papers"));
    }

    #[test]
    fn icij_traverses_entity_officer_to_other_entities_graph() {
        let entity = parse_node_details(&detail_fixture()).unwrap();
        let mut related_nodes = BTreeMap::new();
        related_nodes.insert("87654321".to_string(), officer_detail_fixture());

        let visited = traverse_entity_officer_graph(&entity, &related_nodes);

        assert!(visited.contains(&"12345678".to_string()));
        assert!(visited.contains(&"87654321".to_string()));
        assert!(visited.contains(&"99999999".to_string()));
    }

    #[test]
    fn icij_search_parser_tracks_pagination_for_large_result_sets() {
        let page = parse_search_page(&search_fixture());

        assert_eq!(page.page, 1);
        assert_eq!(page.total_pages, Some(3));
        assert!(page.has_next_page);
    }
}
