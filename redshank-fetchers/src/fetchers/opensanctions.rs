//! OpenSanctions — global sanctions, PEP, and watchlist entity matching.
//!
//! Source: <https://api.opensanctions.org/>
//! Free tier: 100 requests/day. Paid tiers and self-hosting available.
//! Requires an API key passed as the `Authorization: ApiKey <key>` header.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.opensanctions.org";

/// An OpenSanctions entity match result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SanctionsMatch {
    /// Entity ID in OpenSanctions.
    pub id: String,
    /// Schema type (Person, Organization, Company, LegalEntity, etc.).
    pub schema: String,
    /// Match score (0.0–1.0).
    pub score: f64,
    /// Whether this result is a positive/strong match.
    pub is_match: bool,
    /// Source datasets where this entity appears.
    pub datasets: Vec<String>,
    /// Topics (sanction, debarment, pep, crime, wanted).
    pub topics: Vec<String>,
    /// Primary name.
    pub caption: String,
    /// All known names/aliases.
    pub names: Vec<String>,
    /// Known birth dates (for individuals).
    pub birth_dates: Vec<String>,
    /// Known nationalities (for individuals).
    pub nationalities: Vec<String>,
    /// Known countries.
    pub countries: Vec<String>,
    /// Known addresses.
    pub addresses: Vec<String>,
    /// Known identifiers (passport, ID, SWIFT BIC, etc.).
    pub identifiers: Vec<String>,
}

/// Match an entity name against the OpenSanctions database.
///
/// Uses the `/match/default` endpoint with `schema=Thing` for a broad match
/// across all entity types.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or returns a non-2xx status.
pub async fn fetch_matches(
    query: &str,
    api_key: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    let body = serde_json::json!({
        "queries": {
            "entity": {
                "schema": "Thing",
                "properties": {
                    "name": [query]
                }
            }
        }
    });

    let resp = client
        .post(format!("{API_BASE}/match/default"))
        .header("Authorization", format!("ApiKey {api_key}"))
        .json(&body)
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
    let matches = parse_match_response(&json);
    let serialized: Vec<serde_json::Value> = matches
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok())
        .collect();

    let output_path = output_dir.join("opensanctions.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "opensanctions".into(),
        attribution: None,
    })
}

/// Parse the `/match` API response into a list of `SanctionsMatch` records.
#[must_use]
pub fn parse_match_response(json: &serde_json::Value) -> Vec<SanctionsMatch> {
    // The response has shape: { "responses": { "<query_key>": { "results": [...] } } }
    // or equivalently the top-level object can be iterated directly.
    let responses = match json.get("responses") {
        Some(r) => r,
        None => json,
    };

    let mut all_matches = Vec::new();
    if let Some(obj) = responses.as_object() {
        for response_val in obj.values() {
            let results = response_val
                .get("results")
                .and_then(serde_json::Value::as_array);
            if let Some(results) = results {
                all_matches.extend(results.iter().filter_map(parse_match_item));
            }
        }
    }
    all_matches
}

fn str_vec(val: &serde_json::Value, key: &str) -> Vec<String> {
    val.get(key)
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_match_item(item: &serde_json::Value) -> Option<SanctionsMatch> {
    let id = item.get("id")?.as_str()?.to_string();
    let schema = item
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Thing")
        .to_string();
    let score = item
        .get("score")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    let is_match = item
        .get("match")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let caption = item
        .get("caption")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    let datasets = str_vec(item, "datasets");
    let topics = str_vec(item, "topics");

    // Properties are nested under `properties`
    let props = item.get("properties").unwrap_or(&serde_json::Value::Null);
    let names = str_vec(props, "name");
    let birth_dates = str_vec(props, "birthDate");
    let nationalities = str_vec(props, "nationality");
    let countries = str_vec(props, "country");
    let addresses = str_vec(props, "address");
    let identifiers = str_vec(props, "passportNumber")
        .into_iter()
        .chain(str_vec(props, "idNumber"))
        .chain(str_vec(props, "taxNumber"))
        .collect();

    Some(SanctionsMatch {
        id,
        schema,
        score,
        is_match,
        datasets,
        topics,
        caption,
        names,
        birth_dates,
        nationalities,
        countries,
        addresses,
        identifiers,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn match_fixture() -> serde_json::Value {
        serde_json::json!({
            "responses": {
                "entity": {
                    "results": [
                        {
                            "id": "NK-abc123",
                            "schema": "Person",
                            "score": 0.92,
                            "match": true,
                            "caption": "JOHN DOE",
                            "datasets": ["us_ofac_sdn", "eu_fsf"],
                            "topics": ["sanction"],
                            "properties": {
                                "name": ["John Doe", "JOHN DOE"],
                                "birthDate": ["1970-01-01"],
                                "nationality": ["Russian"],
                                "country": ["RU"],
                                "passportNumber": ["P12345678"]
                            }
                        },
                        {
                            "id": "NK-def456",
                            "schema": "Organization",
                            "score": 0.75,
                            "match": false,
                            "caption": "DOE ENTERPRISES LLC",
                            "datasets": ["us_ofac_sdn"],
                            "topics": ["sanction", "debarment"],
                            "properties": {
                                "name": ["Doe Enterprises LLC"],
                                "country": ["IR"],
                                "taxNumber": ["TAX-9876"]
                            }
                        }
                    ]
                }
            }
        })
    }

    #[test]
    fn opensanctions_parses_match_response_extracts_id_score_datasets() {
        let json = match_fixture();
        let matches = parse_match_response(&json);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].id, "NK-abc123");
        assert_eq!(matches[0].schema, "Person");
        assert!((matches[0].score - 0.92).abs() < f64::EPSILON);
        assert!(matches[0].is_match);
        assert!(matches[0].datasets.contains(&"us_ofac_sdn".to_string()));
        assert!(matches[0].topics.contains(&"sanction".to_string()));
    }

    #[test]
    fn opensanctions_extracts_properties_names_dob_nationality() {
        let json = match_fixture();
        let matches = parse_match_response(&json);

        assert_eq!(matches[0].caption, "JOHN DOE");
        assert!(matches[0].names.contains(&"John Doe".to_string()));
        assert_eq!(matches[0].birth_dates, vec!["1970-01-01"]);
        assert!(matches[0].nationalities.contains(&"Russian".to_string()));
        assert!(matches[0].identifiers.contains(&"P12345678".to_string()));
    }

    #[test]
    fn opensanctions_handles_fuzzy_non_match() {
        let json = match_fixture();
        let matches = parse_match_response(&json);

        assert!(!matches[1].is_match);
        assert!((matches[1].score - 0.75).abs() < f64::EPSILON);
        assert_eq!(matches[1].schema, "Organization");
        assert!(matches[1].identifiers.contains(&"TAX-9876".to_string()));
    }

    #[test]
    fn opensanctions_handles_empty_response() {
        let json = serde_json::json!({ "responses": { "entity": { "results": [] } } });
        let matches = parse_match_response(&json);
        assert!(matches.is_empty());
    }
}
