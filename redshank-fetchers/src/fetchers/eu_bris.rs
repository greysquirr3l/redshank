//! EU BRIS search parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const BRIS_PORTAL: &str =
    "https://e-justice.europa.eu/489/EN/business_registers__search_for_a_company_in_the_eu";

/// A normalized BRIS company record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct BrisCompany {
    /// Company name.
    pub company_name: String,
    /// European Unique Identifier.
    pub euid: Option<String>,
    /// National registration number.
    pub registration_number: Option<String>,
    /// Member-state country code.
    pub country_code: Option<String>,
    /// Legal form.
    pub legal_form: Option<String>,
    /// Registered office.
    pub registered_office: Option<String>,
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

/// Parse a BRIS search results fixture.
#[must_use]
pub fn parse_bris_search(document: &str) -> Vec<BrisCompany> {
    let company_names = collect_attr_values(document, "data-company-name");
    let euids = collect_attr_values(document, "data-euid");
    let registration_numbers = collect_attr_values(document, "data-registration-number");
    let countries = collect_attr_values(document, "data-country-code");
    let legal_forms = collect_attr_values(document, "data-legal-form");
    let offices = collect_attr_values(document, "data-registered-office");

    company_names
        .iter()
        .enumerate()
        .map(|(index, company_name)| BrisCompany {
            company_name: company_name.clone(),
            euid: euids.get(index).cloned(),
            registration_number: registration_numbers.get(index).cloned(),
            country_code: countries.get(index).cloned(),
            legal_form: legal_forms.get(index).cloned(),
            registered_office: offices.get(index).cloned(),
        })
        .collect()
}

/// Fetch BRIS search page content.
///
/// # Errors
///
/// Returns `Err` if the request fails.
pub async fn fetch_eu_bris(query: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get(BRIS_PORTAL)
        .query(&[("query", query)])
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

    let body = resp.text().await?;
    let output_path = output_dir.join("eu_bris.ndjson");
    let count = write_ndjson(
        &output_path,
        &[serde_json::json!({"query": query, "body": body})],
    )?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "eu_bris".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn bris_fixture() -> &'static str {
        r#"
        <div data-company-name="Acme Europa GmbH" data-euid="DEHRB12345.B1234" data-registration-number="HRB 1234" data-country-code="DE" data-legal-form="GmbH" data-registered-office="Berlin"></div>
        <div data-company-name="Acme France SARL" data-euid="FRRCS75010001" data-registration-number="750 100 001" data-country-code="FR" data-legal-form="SARL" data-registered-office="Paris"></div>
        "#
    }

    #[test]
    fn eu_bris_fetcher_parses_company_search_across_member_states() {
        let companies = parse_bris_search(bris_fixture());
        assert_eq!(companies.len(), 2);
        assert_eq!(companies[0].country_code.as_deref(), Some("DE"));
        assert_eq!(companies[1].country_code.as_deref(), Some("FR"));
    }

    #[test]
    fn eu_bris_fetcher_extracts_euid() {
        let companies = parse_bris_search(bris_fixture());
        assert_eq!(companies[0].euid.as_deref(), Some("DEHRB12345.B1234"));
        assert_eq!(companies[1].euid.as_deref(), Some("FRRCS75010001"));
    }
}
