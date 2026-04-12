//! Germany Handelsregister parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const HANDELSREGISTER_URL: &str = "https://www.handelsregister.de";

/// A normalized German company register record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GermanyCompanyRecord {
    /// Registered company name.
    pub firma: String,
    /// Legal form.
    pub rechtsform: Option<String>,
    /// Registered seat.
    pub sitz: Option<String>,
    /// Register number.
    pub registernummer: Option<String>,
    /// Business purpose.
    pub gegenstand: Option<String>,
    /// Share capital text.
    pub stammkapital: Option<String>,
    /// Managing directors or board members.
    pub managing_directors: Vec<String>,
}

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let from = haystack.find(start)? + start.len();
    let remainder = haystack.get(from..)?;
    let to = remainder.find(end)?;
    Some(remainder[..to].trim().to_string())
}

fn collect_attr_values(html: &str, attr: &str) -> Vec<String> {
    let marker = format!("{attr}=\"");
    let mut values = Vec::new();
    let mut remainder = html;

    while let Some(idx) = remainder.find(&marker) {
        let after = &remainder[idx + marker.len()..];
        let Some(end_idx) = after.find('"') else {
            break;
        };
        let value = after[..end_idx].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        remainder = &after[end_idx + 1..];
    }

    values
}

/// Parse a Handelsregister company fixture.
#[must_use]
pub fn parse_handelsregister_company(document: &str) -> Option<GermanyCompanyRecord> {
    Some(GermanyCompanyRecord {
        firma: extract_between(document, "data-firma=\"", "\"")?,
        rechtsform: extract_between(document, "data-rechtsform=\"", "\""),
        sitz: extract_between(document, "data-sitz=\"", "\""),
        registernummer: extract_between(document, "data-registernummer=\"", "\""),
        gegenstand: extract_between(document, "data-gegenstand=\"", "\""),
        stammkapital: extract_between(document, "data-stammkapital=\"", "\""),
        managing_directors: collect_attr_values(document, "data-geschaeftsfuehrer"),
    })
}

/// Fetch a Handelsregister page.
///
/// # Errors
///
/// Returns `Err` if the request fails.
pub async fn fetch_germany_handelsregister(
    query: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(HANDELSREGISTER_URL).query(&[("query", query)]).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError { status: status.as_u16(), body });
    }

    let body = resp.text().await?;
    let output_path = output_dir.join("germany_handelsregister.ndjson");
    let count = write_ndjson(&output_path, &[serde_json::json!({"query": query, "body": body})])?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "germany_handelsregister".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn germany_fixture() -> &'static str {
        r#"
        <main data-firma="Acme Europa GmbH" data-rechtsform="GmbH" data-sitz="Berlin" data-registernummer="HRB 123456 B" data-gegenstand="Software and compliance analytics" data-stammkapital="EUR 25.000"></main>
        <div data-geschaeftsfuehrer="Nina Weber"></div>
        <div data-geschaeftsfuehrer="Oskar Klein"></div>
        "#
    }

    #[test]
    fn germany_handelsregister_fetcher_parses_gmbh_company_fixture() {
        let company = parse_handelsregister_company(germany_fixture()).unwrap();
        assert_eq!(company.firma, "Acme Europa GmbH");
        assert_eq!(company.rechtsform.as_deref(), Some("GmbH"));
        assert_eq!(company.sitz.as_deref(), Some("Berlin"));
    }

    #[test]
    fn germany_handelsregister_fetcher_extracts_managing_directors() {
        let company = parse_handelsregister_company(germany_fixture()).unwrap();
        assert_eq!(company.managing_directors.len(), 2);
        assert_eq!(company.managing_directors[0], "Nina Weber");
        assert_eq!(company.managing_directors[1], "Oskar Klein");
    }
}