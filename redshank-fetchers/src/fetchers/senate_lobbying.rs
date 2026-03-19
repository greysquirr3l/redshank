//! Senate Lobbying — SOPR lobbying disclosure data.
//!
//! Source: <https://soprweb.senate.gov/> (ZIP downloads of XML filings per quarter).
//! No pagination — bulk ZIP per quarter.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const DOWNLOAD_BASE: &str = "https://soprweb.senate.gov/index.cfm";

/// Parse registrant and client names from LD-2 XML content.
#[must_use]
pub fn parse_ld2_registrants(xml_content: &str) -> Vec<serde_json::Value> {
    let mut records = Vec::new();
    // Simple tag extraction (not a full XML parser — production would use quick-xml).
    let mut pos = 0;
    while let Some(start) = xml_content[pos..].find("<Registrant>") {
        let abs_start = pos + start;
        if let Some(end) = xml_content[abs_start..].find("</Registrant>") {
            let block = &xml_content[abs_start..abs_start + end + "</Registrant>".len()];
            let name = extract_tag(block, "RegistrantName");
            let client = extract_tag(block, "ClientName");
            let filing_id = extract_tag(block, "FilingID");
            records.push(serde_json::json!({
                "registrant_name": name,
                "client_name": client,
                "filing_id": filing_id,
                "source": "senate-lobbying-ld2"
            }));
            pos = abs_start + end + "</Registrant>".len();
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

/// Fetch Senate lobbying data for a given year and quarter.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_quarter(
    year: u32,
    quarter: u8,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let url = format!(
        "{DOWNLOAD_BASE}?event=getFilingDetails&year={year}&filingPeriod={quarter}&type=LD-2"
    );

    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let body = resp.text().await?;
    let records = parse_ld2_registrants(&body);

    let output_path = output_dir.join(format!("senate_lobbying_{year}_q{quarter}.ndjson"));
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "senate-lobbying".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn senate_lobbying_xml_parser_extracts_registrant_and_client() {
        let xml = r"<Filing>
            <Registrant>
                <RegistrantName>Acme Lobbying LLC</RegistrantName>
                <ClientName>MegaCorp Inc</ClientName>
                <FilingID>F-12345</FilingID>
            </Registrant>
            <Registrant>
                <RegistrantName>Smith &amp; Associates</RegistrantName>
                <ClientName>Widget Co</ClientName>
                <FilingID>F-67890</FilingID>
            </Registrant>
        </Filing>";

        let records = parse_ld2_registrants(xml);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["registrant_name"], "Acme Lobbying LLC");
        assert_eq!(records[0]["client_name"], "MegaCorp Inc");
        assert_eq!(records[1]["filing_id"], "F-67890");
    }
}
