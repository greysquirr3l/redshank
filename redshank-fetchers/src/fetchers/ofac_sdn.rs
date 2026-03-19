//! OFAC SDN — Treasury Specially Designated Nationals list.
//!
//! Source: <https://www.treasury.gov/ofac/downloads/sdn.xml>
//! Bulk XML download, no pagination.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SDN_XML_URL: &str = "https://www.treasury.gov/ofac/downloads/sdn.xml";

/// Parse SDN entity records from the XML content.
#[must_use]
pub fn parse_sdn_xml(xml_content: &str) -> Vec<serde_json::Value> {
    let mut records = Vec::new();
    let mut pos = 0;

    while let Some(start) = xml_content[pos..].find("<sdnEntry>") {
        let abs_start = pos + start;
        if let Some(end) = xml_content[abs_start..].find("</sdnEntry>") {
            let block = &xml_content[abs_start..abs_start + end + "</sdnEntry>".len()];
            let uid = extract_tag(block, "uid");
            let first = extract_tag(block, "firstName");
            let last = extract_tag(block, "lastName");
            let sdn_type = extract_tag(block, "sdnType");
            let program = extract_tag(block, "program");
            let title = extract_tag(block, "title");

            records.push(serde_json::json!({
                "uid": uid,
                "first_name": first,
                "last_name": last,
                "sdn_type": sdn_type,
                "program": program,
                "title": title,
                "source": "ofac-sdn"
            }));
            pos = abs_start + end + "</sdnEntry>".len();
        } else {
            break;
        }
    }
    records
}

fn extract_tag(text: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(start) = text.find(&open) {
        let after = start + open.len();
        if let Some(end) = text[after..].find(&close) {
            return text[after..after + end].to_string();
        }
    }
    String::new()
}

/// Fetch the full OFAC SDN list.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_sdn(output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(SDN_XML_URL).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let body = resp.text().await?;
    let records = parse_sdn_xml(&body);

    let output_path = output_dir.join("ofac_sdn.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "ofac-sdn".into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ofac_sdn_xml_parser_extracts_entity_name_and_program() {
        let xml = r"<sdnList>
            <sdnEntry>
                <uid>12345</uid>
                <firstName>JOHN</firstName>
                <lastName>DOE</lastName>
                <sdnType>Individual</sdnType>
                <program>SDGT</program>
                <title>Director</title>
            </sdnEntry>
            <sdnEntry>
                <uid>67890</uid>
                <firstName></firstName>
                <lastName>ACME SHELL CORP</lastName>
                <sdnType>Entity</sdnType>
                <program>IRAN</program>
                <title></title>
            </sdnEntry>
        </sdnList>";

        let records = parse_sdn_xml(xml);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["last_name"], "DOE");
        assert_eq!(records[0]["program"], "SDGT");
        assert_eq!(records[0]["sdn_type"], "Individual");
        assert_eq!(records[1]["last_name"], "ACME SHELL CORP");
        assert_eq!(records[1]["sdn_type"], "Entity");
    }
}
