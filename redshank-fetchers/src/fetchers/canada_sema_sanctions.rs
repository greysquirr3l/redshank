//! Canada SEMA — Special Economic Measures Act sanctions list.
//!
//! Source: <https://www.international.gc.ca/world-monde/international_relations-relations_internationales/sanctions/consolidated-consolide.aspx?lang=eng>
//! The page provides a downloadable XML/JSON dataset.
//! No authentication required.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// Canada SEMA consolidated sanctions — direct XML download URL.
const SEMA_XML_URL: &str = "https://www.international.gc.ca/world-monde/assets/office_docs/international_relations-relations_internationales/sanctions/sema-lmes.xml";

/// A Canada SEMA sanctions entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SemaEntry {
    /// First name of individual, or entity name.
    pub first_name: String,
    /// Last name (for individuals).
    pub last_name: String,
    /// Combined full name.
    pub full_name: String,
    /// Individual or Entity.
    pub entity_type: String,
    /// Country associated with the listing.
    pub country: String,
    /// Associated sanctions regime (e.g., Russia, Myanmar, Iran).
    pub regime: String,
    /// Date listed under SEMA.
    pub listed_on: Option<String>,
    /// Aliases.
    pub aliases: Vec<String>,
}

/// Fetch the Canada SEMA consolidated sanctions list (XML).
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or returns a non-2xx status.
pub async fn fetch_sema_sanctions(output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(SEMA_XML_URL).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let xml = resp.text().await?;
    let records = parse_sema_xml(&xml);
    let serialized: Vec<serde_json::Value> = records
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let output_path = output_dir.join("canada_sema_sanctions.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "canada_sema_sanctions".into(),
        attribution: None,
    })
}

/// Parse the SEMA XML into a list of sanctions entries.
///
/// The XML uses `<individual>` and `<entity>` tags within `<schedule>` blocks.
#[must_use]
pub fn parse_sema_xml(xml: &str) -> Vec<SemaEntry> {
    let mut entries = Vec::new();
    entries.extend(parse_sema_persons(xml));
    entries.extend(parse_sema_entities(xml));
    entries
}

fn parse_sema_persons(xml: &str) -> Vec<SemaEntry> {
    xml.split("<individual>")
        .skip(1)
        .map(|chunk| {
            let end = chunk.find("</individual>").unwrap_or(chunk.len());
            let block = &chunk[..end];

            let first_name = extract_tag(block, "givenName")
                .or_else(|| extract_tag(block, "firstName"))
                .unwrap_or_default();
            let last_name = extract_tag(block, "familyName")
                .or_else(|| extract_tag(block, "lastName"))
                .unwrap_or_default();
            let full_name = if first_name.is_empty() {
                last_name.clone()
            } else {
                format!("{first_name} {last_name}")
            };

            let country = extract_tag(block, "country").unwrap_or_default();
            let regime = extract_tag(block, "schedule")
                .or_else(|| extract_tag(block, "regime"))
                .unwrap_or_default();
            let listed_on =
                extract_tag(block, "dateOfListing").or_else(|| extract_tag(block, "listedOn"));

            let aliases = collect_tags(block, "alias");

            SemaEntry {
                first_name,
                last_name,
                full_name,
                entity_type: "Individual".to_string(),
                country,
                regime,
                listed_on,
                aliases,
            }
        })
        .collect()
}

fn parse_sema_entities(xml: &str) -> Vec<SemaEntry> {
    xml.split("<entity>")
        .skip(1)
        .map(|chunk| {
            let end = chunk.find("</entity>").unwrap_or(chunk.len());
            let block = &chunk[..end];

            let name = extract_tag(block, "name").unwrap_or_default();

            let country = extract_tag(block, "country").unwrap_or_default();
            let regime = extract_tag(block, "schedule")
                .or_else(|| extract_tag(block, "regime"))
                .unwrap_or_default();
            let listed_on =
                extract_tag(block, "dateOfListing").or_else(|| extract_tag(block, "listedOn"));

            let aliases = collect_tags(block, "alias");

            SemaEntry {
                first_name: String::new(),
                last_name: name.clone(),
                full_name: name,
                entity_type: "Entity".to_string(),
                country,
                regime,
                listed_on,
                aliases,
            }
        })
        .collect()
}

fn extract_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)?;
    let value = block[start..start + end].trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn collect_tags(block: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut results = Vec::new();
    let mut remaining = block;

    while let Some(start) = remaining.find(&open) {
        let rest = &remaining[start + open.len()..];
        if let Some(end) = rest.find(&close) {
            let value = rest[..end].trim().to_string();
            if !value.is_empty() {
                results.push(value);
            }
            remaining = &rest[end + close.len()..];
        } else {
            break;
        }
    }
    results
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const XML_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sanctionsList>
  <individual>
    <givenName>Vladimir</givenName>
    <familyName>PUTIN</familyName>
    <country>Russia</country>
    <schedule>Russia</schedule>
    <dateOfListing>2022-02-28</dateOfListing>
    <alias>Vladimir Vladimirovich PUTIN</alias>
  </individual>
  <individual>
    <givenName>Sergei</givenName>
    <familyName>LAVROV</familyName>
    <country>Russia</country>
    <schedule>Russia</schedule>
    <dateOfListing>2022-02-28</dateOfListing>
  </individual>
  <entity>
    <name>SBERBANK</name>
    <country>Russia</country>
    <schedule>Russia</schedule>
    <dateOfListing>2022-03-12</dateOfListing>
    <alias>Sberbank of Russia PJSC</alias>
    <alias>Savings Bank of Russia</alias>
  </entity>
</sanctionsList>
"#;

    #[test]
    fn sema_parses_individuals_extracts_names_regime_listing_date() {
        let entries = parse_sema_xml(XML_FIXTURE);
        let individuals: Vec<_> = entries
            .iter()
            .filter(|e| e.entity_type == "Individual")
            .collect();

        assert_eq!(individuals.len(), 2);
        assert_eq!(individuals[0].first_name, "Vladimir");
        assert_eq!(individuals[0].last_name, "PUTIN");
        assert_eq!(individuals[0].full_name, "Vladimir PUTIN");
        assert_eq!(individuals[0].regime, "Russia");
        assert_eq!(individuals[0].listed_on.as_deref(), Some("2022-02-28"));
        assert!(
            individuals[0]
                .aliases
                .contains(&"Vladimir Vladimirovich PUTIN".to_string())
        );
    }

    #[test]
    fn sema_parses_entity_with_multiple_aliases() {
        let entries = parse_sema_xml(XML_FIXTURE);
        let entities: Vec<_> = entries
            .iter()
            .filter(|e| e.entity_type == "Entity")
            .collect();

        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].full_name, "SBERBANK");
        assert_eq!(entities[0].country, "Russia");
        assert_eq!(entities[0].aliases.len(), 2);
        assert!(
            entities[0]
                .aliases
                .contains(&"Savings Bank of Russia".to_string())
        );
    }

    #[test]
    fn sema_handles_empty_xml() {
        let entries = parse_sema_xml("<sanctionsList></sanctionsList>");
        assert!(entries.is_empty());
    }

    #[test]
    fn sema_handles_regime_and_country_correctly() {
        let entries = parse_sema_xml(XML_FIXTURE);
        assert!(entries.iter().all(|e| e.country == "Russia"));
        assert!(entries.iter().all(|e| e.regime == "Russia"));
    }
}
