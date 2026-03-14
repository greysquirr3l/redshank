//! EU CFSP/RELEX Consolidated Financial Sanctions List.
//!
//! Source: `https://webgate.ec.europa.eu/fsd/fsf/public/files/xmlFullSanctionsList_1_1/content`
//! No auth required. Bulk XML download.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SANCTIONS_URL: &str =
    "https://webgate.ec.europa.eu/fsd/fsf/public/files/xmlFullSanctionsList_1_1/content";

/// Fetch and parse the EU consolidated sanctions list.
pub async fn fetch_eu_sanctions(
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(SANCTIONS_URL).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    let xml = resp.text().await?;
    let records = parse_eu_sanctions_xml(&xml);

    let output_path = output_dir.join("eu_sanctions.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "eu_sanctions".into(),
    })
}

/// Parse the EU sanctions XML, extracting subject records.
///
/// Handles both `<subjectType>person</subjectType>` and `<subjectType>enterprise</subjectType>`.
pub fn parse_eu_sanctions_xml(xml: &str) -> Vec<serde_json::Value> {
    let mut records = Vec::new();

    for chunk in xml.split("<sanctionEntity").skip(1) {
        let end = chunk.find("</sanctionEntity>").unwrap_or(chunk.len());
        let block = &chunk[..end];

        let subject_type = extract_tag(block, "subjectType");
        let regulation_summary = extract_tag(block, "regulationSummary");
        let programme = extract_tag(block, "programme");

        // Collect name aliases
        let name_aliases: Vec<serde_json::Value> = block
            .split("<nameAlias")
            .skip(1)
            .map(|a| {
                let end = a.find("/>").or_else(|| a.find("</nameAlias>")).unwrap_or(a.len());
                let alias_block = &a[..end];
                let whole_name = extract_attr(alias_block, "wholeName");
                let last_name = extract_attr(alias_block, "lastName");
                let first_name = extract_attr(alias_block, "firstName");
                serde_json::json!({
                    "wholeName": whole_name,
                    "lastName": last_name,
                    "firstName": first_name,
                })
            })
            .collect();

        // Collect identification details
        let identifications: Vec<serde_json::Value> = block
            .split("<identification")
            .skip(1)
            .map(|i| {
                let end = i.find("/>").or_else(|| i.find("</identification>")).unwrap_or(i.len());
                let id_block = &i[..end];
                let number = extract_attr(id_block, "number");
                let diplomatic_info = extract_attr(id_block, "diplomaticInformation");
                let id_type = extract_attr(id_block, "identificationTypeDescription");
                serde_json::json!({
                    "number": number,
                    "type": id_type,
                    "diplomaticInformation": diplomatic_info,
                })
            })
            .collect();

        records.push(serde_json::json!({
            "subject_type": subject_type,
            "regulation_summary": regulation_summary,
            "programme": programme,
            "name_aliases": name_aliases,
            "identifications": identifications,
        }));
    }

    records
}

fn extract_tag(xml: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    xml.find(&open)
        .and_then(|start| {
            let content_start = start + open.len();
            xml[content_start..]
                .find(&close)
                .map(|end| xml[content_start..content_start + end].trim().to_owned())
        })
        .unwrap_or_default()
}

fn extract_attr(xml: &str, attr: &str) -> String {
    let pattern = format!("{attr}=\"");
    xml.find(&pattern)
        .map(|start| {
            let value_start = start + pattern.len();
            xml[value_start..]
                .find('"')
                .map(|end| xml[value_start..value_start + end].to_owned())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eu_sanctions_xml_handles_individual_and_entity() {
        let xml = r#"
<sanctionsList>
  <sanctionEntity logicalId="1">
    <subjectType>person</subjectType>
    <regulationSummary>Council Regulation (EU) 2024/123</regulationSummary>
    <programme>SYRIA</programme>
    <nameAlias wholeName="John Doe" firstName="John" lastName="Doe"/>
    <nameAlias wholeName="Johnny D" firstName="Johnny" lastName="D"/>
    <identification number="AB123456" identificationTypeDescription="passport"/>
  </sanctionEntity>
  <sanctionEntity logicalId="2">
    <subjectType>enterprise</subjectType>
    <regulationSummary>Council Regulation (EU) 2023/456</regulationSummary>
    <programme>RUSSIA</programme>
    <nameAlias wholeName="Acme Trading Ltd" firstName="" lastName="Acme Trading Ltd"/>
  </sanctionEntity>
</sanctionsList>
"#;
        let records = parse_eu_sanctions_xml(xml);
        assert_eq!(records.len(), 2);

        // Person
        assert_eq!(records[0]["subject_type"], "person");
        assert_eq!(records[0]["programme"], "SYRIA");
        let aliases = records[0]["name_aliases"].as_array().unwrap();
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0]["wholeName"], "John Doe");
        let ids = records[0]["identifications"].as_array().unwrap();
        assert_eq!(ids[0]["number"], "AB123456");

        // Enterprise
        assert_eq!(records[1]["subject_type"], "enterprise");
        assert_eq!(records[1]["programme"], "RUSSIA");
    }
}
