//! Wikidata SPARQL — Entity disambiguation via Wikidata Query Service.
//!
//! Endpoint: POST `https://query.wikidata.org/sparql`
//! Content-Type: application/x-www-form-urlencoded
//! Accept: application/sparql-results+json
//! No auth required.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SPARQL_ENDPOINT: &str = "https://query.wikidata.org/sparql";

/// SPARQL query: find entities by label, returning QID, description, and instance-of.
pub const ENTITY_SEARCH_QUERY: &str = r#"
SELECT ?item ?itemLabel ?itemDescription ?instanceOfLabel WHERE {
  ?item rdfs:label "{NAME}"@en .
  OPTIONAL { ?item wdt:P31 ?instanceOf . }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en" . }
}
LIMIT 50
"#;

/// SPARQL query: find board memberships for a person (P463=member of, P108=employer).
pub const BOARD_MEMBERSHIP_QUERY: &str = r#"
SELECT ?org ?orgLabel ?roleLabel ?startDate ?endDate WHERE {
  wd:{QID} wdt:P463|wdt:P108 ?org .
  OPTIONAL { wd:{QID} p:P463|p:P108 ?stmt . ?stmt ps:P463|ps:P108 ?org . ?stmt pq:P580 ?startDate . ?stmt pq:P582 ?endDate . }
  OPTIONAL { ?stmt pq:P39 ?role . }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en" . }
}
"#;

/// SPARQL query: find subsidiary tree for a company (P355=subsidiary, P749=parent).
pub const SUBSIDIARY_TREE_QUERY: &str = r#"
SELECT ?subsidiary ?subsidiaryLabel ?parentLabel WHERE {
  wd:{QID} wdt:P355* ?subsidiary .
  OPTIONAL { ?subsidiary wdt:P749 ?parent . }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en" . }
}
"#;

/// Execute a SPARQL query against Wikidata and write results as NDJSON.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_wikidata_sparql(
    sparql_query: &str,
    output_dir: &Path,
    output_filename: &str,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    let resp = client
        .post(SPARQL_ENDPOINT)
        .header("Accept", "application/sparql-results+json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("query={sparql_query}"))
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
    let bindings = json
        .get("results")
        .and_then(|r| r.get("bindings"))
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();

    let records = parse_sparql_bindings(&bindings);

    let output_path = output_dir.join(output_filename);
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "wikidata".into(),
    })
}

/// Build an entity search query for the given name.
#[must_use]
pub fn build_entity_query(name: &str) -> String {
    ENTITY_SEARCH_QUERY.replace("{NAME}", &name.replace('"', "\\\""))
}

/// Build a board membership query for the given Wikidata QID.
#[must_use]
pub fn build_board_query(qid: &str) -> String {
    BOARD_MEMBERSHIP_QUERY.replace("{QID}", qid)
}

/// Build a subsidiary tree query for the given Wikidata QID.
#[must_use]
pub fn build_subsidiary_query(qid: &str) -> String {
    SUBSIDIARY_TREE_QUERY.replace("{QID}", qid)
}

/// Flatten SPARQL bindings into simple key-value records.
fn parse_sparql_bindings(bindings: &[serde_json::Value]) -> Vec<serde_json::Value> {
    bindings
        .iter()
        .filter_map(|binding| {
            let obj = binding.as_object()?;
            let mut record = serde_json::Map::new();
            for (key, val) in obj {
                let value = val
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_owned();
                record.insert(key.clone(), serde_json::Value::String(value));
            }
            Some(serde_json::Value::Object(record))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn wikidata_sparql_parses_bindings_response() {
        let mock = serde_json::json!({
            "results": {
                "bindings": [
                    {
                        "item": {"type": "uri", "value": "http://www.wikidata.org/entity/Q312"},
                        "itemLabel": {"type": "literal", "value": "Apple Inc."},
                        "itemDescription": {"type": "literal", "value": "American multinational technology company"},
                        "instanceOfLabel": {"type": "literal", "value": "public company"}
                    },
                    {
                        "item": {"type": "uri", "value": "http://www.wikidata.org/entity/Q95"},
                        "itemLabel": {"type": "literal", "value": "Google"},
                        "itemDescription": {"type": "literal", "value": "American technology company"},
                        "instanceOfLabel": {"type": "literal", "value": "subsidiary"}
                    }
                ]
            }
        });
        let bindings = mock["results"]["bindings"].as_array().unwrap();
        let records = parse_sparql_bindings(bindings);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["itemLabel"], "Apple Inc.");
        assert!(records[0]["item"].as_str().unwrap().contains("Q312"));
        assert_eq!(records[1]["instanceOfLabel"], "subsidiary");
    }

    #[test]
    fn wikidata_builds_entity_query_with_escaped_name() {
        let query = build_entity_query("Acme \"Corp\"");
        assert!(query.contains(r#"Acme \"Corp\""#));
    }

    #[test]
    fn wikidata_builds_board_query_with_qid() {
        let query = build_board_query("Q312");
        assert!(query.contains("wd:Q312"));
    }
}
