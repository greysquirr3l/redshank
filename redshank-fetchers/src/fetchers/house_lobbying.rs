//! House Lobbying — House Clerk LD-1/LD-2 lobbying disclosures.
//!
//! API: `https://clerkapi.house.gov/Lobbying/`
//! Provides LD-1 registrations and LD-2 activity reports in XML format.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://clerkapi.house.gov/Lobbying";

/// Fetch House lobbying disclosures matching the given registrant or client name.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_house_lobbying(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let resp = client
            .get(format!("{API_BASE}/Registrations"))
            .query(&[("registrantName", query), ("page", &page.to_string())])
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

        let text = resp.text().await?;
        let records = parse_house_registrations(&text);

        if records.is_empty() {
            break;
        }
        all_records.extend(records);

        if page >= max {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("house_lobbying.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "house_lobbying".into(),
        attribution: None,
    })
}

/// Parse House lobbying XML response into JSON value records.
///
/// Extracts `<Registration>` elements with registrant, client, and issue codes.
#[must_use]
pub fn parse_house_registrations(xml: &str) -> Vec<serde_json::Value> {
    let mut records = Vec::new();

    for chunk in xml.split("<Registration>").skip(1) {
        let end = chunk.find("</Registration>").unwrap_or(chunk.len());
        let block = &chunk[..end];

        let registrant = extract_tag(block, "RegistrantName");
        let client = extract_tag(block, "ClientName");
        let filing_year = extract_tag(block, "FilingYear");
        let filing_type = extract_tag(block, "FilingType");
        let issue_codes = extract_tag(block, "GeneralIssueCodeDisplay");

        records.push(serde_json::json!({
            "registrant": registrant,
            "client": client,
            "filing_year": filing_year,
            "filing_type": filing_type,
            "issue_codes": issue_codes,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn house_lobbying_parses_registration_xml() {
        let xml = r"
<Registrations>
  <Registration>
    <RegistrantName>PATTON BOGGS LLP</RegistrantName>
    <ClientName>ACME CORP</ClientName>
    <FilingYear>2024</FilingYear>
    <FilingType>LD-2</FilingType>
    <GeneralIssueCodeDisplay>TAX;TRD</GeneralIssueCodeDisplay>
  </Registration>
  <Registration>
    <RegistrantName>AKIN GUMP</RegistrantName>
    <ClientName>SHELL LLC</ClientName>
    <FilingYear>2024</FilingYear>
    <FilingType>LD-1</FilingType>
    <GeneralIssueCodeDisplay>DEF</GeneralIssueCodeDisplay>
  </Registration>
</Registrations>
";
        let records = parse_house_registrations(xml);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["registrant"], "PATTON BOGGS LLP");
        assert_eq!(records[0]["client"], "ACME CORP");
        assert_eq!(records[0]["issue_codes"], "TAX;TRD");
        assert_eq!(records[1]["filing_type"], "LD-1");
    }
}
