//! PACER parser and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// PACER login credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacerCredentials {
    pub username: String,
    pub password: String,
}

/// A PACER party/case search result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PacerCase {
    pub case_number: String,
    pub case_title: String,
    pub court: Option<String>,
    pub filed_date: Option<String>,
    pub parties: Vec<String>,
}

/// A PACER docket entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PacerDocketEntry {
    pub number: String,
    pub filed_at: Option<String>,
    pub description: String,
    pub pdf_url: Option<String>,
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

/// PACER login config can be loaded without attempting authentication.
#[must_use]
pub fn login_payload(credentials: &PacerCredentials) -> Vec<(&str, String)> {
    vec![
        ("login", credentials.username.clone()),
        ("key", credentials.password.clone()),
    ]
}

/// Parse PACER party/case search HTML fixture.
#[must_use]
pub fn parse_case_search_results(html: &str) -> Vec<PacerCase> {
    let case_numbers = collect_attr_values(html, "data-case-number");
    let case_titles = collect_attr_values(html, "data-case-title");
    let courts = collect_attr_values(html, "data-case-court");
    let filed_dates = collect_attr_values(html, "data-case-filed");
    let party_values = collect_attr_values(html, "data-case-party");

    case_numbers
        .iter()
        .enumerate()
        .map(|(index, case_number)| PacerCase {
            case_number: case_number.clone(),
            case_title: case_titles.get(index).cloned().unwrap_or_default(),
            court: courts.get(index).cloned(),
            filed_date: filed_dates.get(index).cloned(),
            parties: party_values
                .iter()
                .filter_map(|party| {
                    let prefix = format!("{}|", index + 1);
                    party.strip_prefix(&prefix).map(str::to_string)
                })
                .collect(),
        })
        .collect()
}

/// Parse PACER docket entry HTML fixture.
#[must_use]
pub fn parse_docket_entries(html: &str) -> Vec<PacerDocketEntry> {
    let numbers = collect_attr_values(html, "data-docket-number");
    let filed_dates = collect_attr_values(html, "data-docket-filed");
    let descriptions = collect_attr_values(html, "data-docket-description");
    let pdf_urls = collect_attr_values(html, "data-docket-pdf");

    numbers
        .iter()
        .enumerate()
        .map(|(index, number)| PacerDocketEntry {
            number: number.clone(),
            filed_at: filed_dates.get(index).cloned(),
            description: descriptions.get(index).cloned().unwrap_or_default(),
            pdf_url: pdf_urls.get(index).cloned(),
        })
        .collect()
}

/// Fetch a PACER case locator page.
///
/// # Errors
///
/// Returns `Err` if the request fails or the response status is non-success.
pub async fn fetch_case_locator(
    query_url: &str,
    credentials: &PacerCredentials,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get(query_url)
        .basic_auth(&credentials.username, Some(&credentials.password))
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

    let html = resp.text().await?;
    let cases = parse_case_search_results(&html)
        .into_iter()
        .map(|case| serde_json::to_value(case).map_err(|err| FetchError::Parse(err.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    let output_path = output_dir.join("pacer_cases.ndjson");
    let count = write_ndjson(&output_path, &cases)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "pacer".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn party_search_fixture() -> &'static str {
        r#"
        <html>
          <body>
            <div data-case-number="1:24-cv-01234"></div>
            <div data-case-title="Acme Corp v. Northwind Holdings"></div>
            <div data-case-court="S.D.N.Y."></div>
            <div data-case-filed="2024-02-10"></div>
            <div data-case-party="1|Acme Corp"></div>
            <div data-case-party="1|Northwind Holdings"></div>
          </body>
        </html>
        "#
    }

    fn docket_fixture() -> &'static str {
        r#"
        <html>
          <body>
            <div data-docket-number="1"></div>
            <div data-docket-filed="2024-02-10"></div>
            <div data-docket-description="Complaint"></div>
            <div data-docket-pdf="https://pacer.example.com/doc1.pdf"></div>
            <div data-docket-number="7"></div>
            <div data-docket-filed="2024-03-14"></div>
            <div data-docket-description="Order granting motion to dismiss"></div>
            <div data-docket-pdf="https://pacer.example.com/doc7.pdf"></div>
          </body>
        </html>
        "#
    }

    #[test]
    fn pacer_authenticates_and_searches_case_docket_config() {
        let credentials = PacerCredentials {
            username: "user@example.com".to_string(),
            password: "secret".to_string(),
        };
        let payload = login_payload(&credentials);

        assert_eq!(payload[0], ("login", "user@example.com".to_string()));
        assert_eq!(payload[1], ("key", "secret".to_string()));
    }

    #[test]
    fn pacer_parses_party_search_results() {
        let cases = parse_case_search_results(party_search_fixture());

        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].case_number, "1:24-cv-01234");
        assert_eq!(cases[0].parties.len(), 2);
        assert!(cases[0].parties.contains(&"Acme Corp".to_string()));
    }

    #[test]
    fn pacer_downloads_docket_entries_pdf_links() {
        let entries = parse_docket_entries(docket_fixture());

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].number, "1");
        assert_eq!(entries[1].description, "Order granting motion to dismiss");
        assert_eq!(
            entries[0].pdf_url.as_deref(),
            Some("https://pacer.example.com/doc1.pdf")
        );
    }
}
