//! UN Security Council Consolidated Sanctions List.
//!
//! Source: `https://scsanctions.un.org/resources/xml/en/consolidated.xml`
//! No auth required. Bulk XML download.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SANCTIONS_URL: &str = "https://scsanctions.un.org/resources/xml/en/consolidated.xml";

/// Fetch and parse the UN consolidated sanctions list.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_un_sanctions(output_dir: &Path) -> Result<FetchOutput, FetchError> {
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
    let records = parse_un_sanctions_xml(&xml);

    let output_path = output_dir.join("un_sanctions.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "un_sanctions".into(),
        attribution: None,
    })
}

/// Parse the UN sanctions XML, extracting individual and entity records.
///
/// Looks for `<INDIVIDUAL>` and `<ENTITY>` elements with name components,
/// aliases, identifiers (passport, national ID), and listing info.
#[must_use]
pub fn parse_un_sanctions_xml(xml: &str) -> Vec<serde_json::Value> {
    let mut records = Vec::new();

    for (tag, entity_type) in [("INDIVIDUAL", "individual"), ("ENTITY", "entity")] {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        for chunk in xml.split(&open).skip(1) {
            let end = chunk.find(&close).unwrap_or(chunk.len());
            let block = &chunk[..end];

            let first_name = extract_tag(block, "FIRST_NAME");
            let second_name = extract_tag(block, "SECOND_NAME");
            let third_name = extract_tag(block, "THIRD_NAME");
            let un_list_type = extract_tag(block, "UN_LIST_TYPE");
            let reference_number = extract_tag(block, "REFERENCE_NUMBER");
            let listed_on = extract_tag(block, "LISTED_ON");
            let comments = extract_tag(block, "COMMENTS1");

            // Collect aliases
            let aliases: Vec<String> = block
                .split("<ALIAS_NAME>")
                .skip(1)
                .filter_map(|a| a.find("</ALIAS_NAME>").map(|e| a[..e].trim().to_owned()))
                .collect();

            // Collect document identifiers (passport, national ID)
            let documents: Vec<serde_json::Value> = block
                .split("<DOCUMENT>")
                .skip(1)
                .map(|d| {
                    let end = d.find("</DOCUMENT>").unwrap_or(d.len());
                    let doc_block = &d[..end];
                    serde_json::json!({
                        "type": extract_tag(doc_block, "TYPE_OF_DOCUMENT"),
                        "number": extract_tag(doc_block, "NUMBER"),
                        "country": extract_tag(doc_block, "COUNTRY_OF_ISSUE"),
                    })
                })
                .collect();

            let name = [
                first_name.as_str(),
                second_name.as_str(),
                third_name.as_str(),
            ]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");

            records.push(serde_json::json!({
                "entity_type": entity_type,
                "name": name,
                "un_list_type": un_list_type,
                "reference_number": reference_number,
                "listed_on": listed_on,
                "comments": comments,
                "aliases": aliases,
                "documents": documents,
            }));
        }
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn un_sanctions_xml_parser_extracts_aliases_and_identifiers() {
        let xml = r"
<CONSOLIDATED_LIST>
  <INDIVIDUALS>
    <INDIVIDUAL>
      <DATAID>123</DATAID>
      <FIRST_NAME>JOHN</FIRST_NAME>
      <SECOND_NAME>DOE</SECOND_NAME>
      <THIRD_NAME></THIRD_NAME>
      <UN_LIST_TYPE>Al-Qaida</UN_LIST_TYPE>
      <REFERENCE_NUMBER>QDi.001</REFERENCE_NUMBER>
      <LISTED_ON>2001-10-08</LISTED_ON>
      <COMMENTS1>Listed for association</COMMENTS1>
      <ALIAS_NAME>JOHNNY D</ALIAS_NAME>
      <ALIAS_NAME>J DOE</ALIAS_NAME>
      <DOCUMENT>
        <TYPE_OF_DOCUMENT>Passport</TYPE_OF_DOCUMENT>
        <NUMBER>AB123456</NUMBER>
        <COUNTRY_OF_ISSUE>AF</COUNTRY_OF_ISSUE>
      </DOCUMENT>
    </INDIVIDUAL>
  </INDIVIDUALS>
  <ENTITIES>
    <ENTITY>
      <DATAID>456</DATAID>
      <FIRST_NAME>ACME TRADING CO</FIRST_NAME>
      <SECOND_NAME></SECOND_NAME>
      <THIRD_NAME></THIRD_NAME>
      <UN_LIST_TYPE>Taliban</UN_LIST_TYPE>
      <REFERENCE_NUMBER>QDe.002</REFERENCE_NUMBER>
      <LISTED_ON>2002-03-15</LISTED_ON>
      <COMMENTS1>Front company</COMMENTS1>
    </ENTITY>
  </ENTITIES>
</CONSOLIDATED_LIST>
";
        let records = parse_un_sanctions_xml(xml);
        assert_eq!(records.len(), 2);

        // Individual
        assert_eq!(records[0]["entity_type"], "individual");
        assert_eq!(records[0]["name"], "JOHN DOE");
        let aliases = records[0]["aliases"].as_array().unwrap();
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0], "JOHNNY D");
        let docs = records[0]["documents"].as_array().unwrap();
        assert_eq!(docs[0]["type"], "Passport");
        assert_eq!(docs[0]["number"], "AB123456");

        // Entity
        assert_eq!(records[1]["entity_type"], "entity");
        assert_eq!(records[1]["name"], "ACME TRADING CO");
        assert_eq!(records[1]["un_list_type"], "Taliban");
    }
}
