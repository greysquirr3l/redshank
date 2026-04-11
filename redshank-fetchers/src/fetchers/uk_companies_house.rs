//! UK Companies House — corporate registry and beneficial ownership data.
//!
//! Source: <https://developer.company-information.service.gov.uk/>
//! Requires a free API key from the Companies House developer portal.
//! Auth: HTTP Basic with API key as the username, empty password.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const BASE_URL: &str = "https://api.company-information.service.gov.uk";

/// A Companies House company record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompanyRecord {
    /// Unique Companies House identifier.
    pub company_number: String,
    /// Registered company name.
    pub company_name: String,
    /// Company type (ltd, plc, llp, etc.).
    pub company_type: Option<String>,
    /// Registration status (active, dissolved, liquidation, etc.).
    pub company_status: Option<String>,
    /// Date of incorporation.
    pub date_of_creation: Option<String>,
    /// Registered office address as a single formatted string.
    pub registered_office_address: Option<String>,
    /// Standard Industrial Classification codes.
    pub sic_codes: Vec<String>,
    /// Officers (directors, secretaries).
    pub officers: Vec<OfficerRecord>,
    /// Persons with Significant Control (beneficial owners).
    pub pscs: Vec<PscRecord>,
}

/// An officer (director or secretary) of a company.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OfficerRecord {
    /// Officer name.
    pub name: String,
    /// Officer role (director, secretary, etc.).
    pub officer_role: String,
    /// Date appointed.
    pub appointed_on: Option<String>,
    /// Date resigned, if applicable.
    pub resigned_on: Option<String>,
    /// Declared nationality.
    pub nationality: Option<String>,
    /// Declared occupation.
    pub occupation: Option<String>,
}

/// A Person with Significant Control (UK equivalent of FinCEN BOI).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PscRecord {
    /// PSC name.
    pub name: String,
    /// Nature of control (e.g. ownership-of-shares-75-to-100-percent).
    pub natures_of_control: Vec<String>,
    /// Date the PSC was notified.
    pub notified_on: Option<String>,
    /// Date ceased, if applicable.
    pub ceased_on: Option<String>,
    /// Country of residence.
    pub country_of_residence: Option<String>,
    /// Date of birth (month/year only per UK regulations).
    pub date_of_birth: Option<String>,
}

/// Search Companies House for companies matching `query` and enrich each
/// result with officers and PSCs.
///
/// # Errors
///
/// Returns `Err` if an HTTP request fails or returns a non-2xx status.
pub async fn fetch_companies(
    query: &str,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_results: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let items_per_page = max_results.min(100);

    // Search for companies
    let resp = client
        .get(format!("{BASE_URL}/search/companies"))
        .basic_auth(api_key, Option::<&str>::None)
        .query(&[
            ("q", query),
            ("items_per_page", &items_per_page.to_string()),
        ])
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

    let json: serde_json::Value = resp.json().await?;
    let companies = parse_search_results(&json);
    let mut all_records = Vec::new();

    for mut company in companies {
        rate_limit_delay(rate_limit_ms).await;
        let num = company.company_number.clone();

        // Fetch officers
        let officers = fetch_officers(&client, api_key, &num, rate_limit_ms).await;
        company.officers = officers.unwrap_or_default();

        rate_limit_delay(rate_limit_ms).await;

        // Fetch PSCs
        let pscs = fetch_pscs(&client, api_key, &num).await;
        company.pscs = pscs.unwrap_or_default();

        let record = serde_json::to_value(&company).map_err(|e| FetchError::Parse(e.to_string()))?;
        all_records.push(record);
    }

    let output_path = output_dir.join("uk_companies_house.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "uk_companies_house".into(),
        attribution: None,
    })
}

/// Parse the company search results from a Companies House API response.
#[must_use]
pub fn parse_search_results(json: &serde_json::Value) -> Vec<CompanyRecord> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().filter_map(parse_company_item).collect())
        .unwrap_or_default()
}

/// Parse officers from a Companies House officers response.
#[must_use]
pub fn parse_officers(json: &serde_json::Value) -> Vec<OfficerRecord> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().map(parse_officer_item).collect())
        .unwrap_or_default()
}

/// Parse PSCs from a Companies House PSC list response.
#[must_use]
pub fn parse_pscs(json: &serde_json::Value) -> Vec<PscRecord> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().map(parse_psc_item).collect())
        .unwrap_or_default()
}

fn parse_company_item(item: &serde_json::Value) -> Option<CompanyRecord> {
    let company_number = item
        .get("company_number")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let company_name = item
        .get("title")
        .or_else(|| item.get("company_name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    let company_type = item
        .get("company_type")
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let company_status = item
        .get("company_status")
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let date_of_creation = item
        .get("date_of_creation")
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let registered_office_address = parse_address(item.get("address").or_else(|| {
        item.get("registered_office_address")
    }));

    let sic_codes = item
        .get("sic_codes")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    Some(CompanyRecord {
        company_number,
        company_name,
        company_type,
        company_status,
        date_of_creation,
        registered_office_address,
        sic_codes,
        officers: Vec::new(),
        pscs: Vec::new(),
    })
}

fn parse_address(addr: Option<&serde_json::Value>) -> Option<String> {
    let a = addr?;
    let parts: Vec<&str> = [
        a.get("premises").and_then(serde_json::Value::as_str),
        a.get("address_line_1").and_then(serde_json::Value::as_str),
        a.get("address_line_2").and_then(serde_json::Value::as_str),
        a.get("locality").and_then(serde_json::Value::as_str),
        a.get("region").and_then(serde_json::Value::as_str),
        a.get("postal_code").and_then(serde_json::Value::as_str),
        a.get("country").and_then(serde_json::Value::as_str),
    ]
    .into_iter()
    .flatten()
    .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn parse_officer_item(item: &serde_json::Value) -> OfficerRecord {
    OfficerRecord {
        name: item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        officer_role: item
            .get("officer_role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        appointed_on: item
            .get("appointed_on")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        resigned_on: item
            .get("resigned_on")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        nationality: item
            .get("nationality")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        occupation: item
            .get("occupation")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    }
}

fn parse_psc_item(item: &serde_json::Value) -> PscRecord {
    let natures_of_control = item
        .get("natures_of_control")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let dob = item.get("date_of_birth").map(|d| {
        let month = d.get("month").and_then(serde_json::Value::as_u64).unwrap_or(0);
        let year = d.get("year").and_then(serde_json::Value::as_u64).unwrap_or(0);
        format!("{month:02}/{year}")
    });

    PscRecord {
        name: item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        natures_of_control,
        notified_on: item
            .get("notified_on")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        ceased_on: item
            .get("ceased_on")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        country_of_residence: item
            .get("country_of_residence")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        date_of_birth: dob,
    }
}

async fn fetch_officers(
    client: &reqwest::Client,
    api_key: &str,
    company_number: &str,
    rate_limit_ms: u64,
) -> Option<Vec<OfficerRecord>> {
    rate_limit_delay(rate_limit_ms).await;
    let resp = client
        .get(format!("{BASE_URL}/company/{company_number}/officers"))
        .basic_auth(api_key, Option::<&str>::None)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    Some(parse_officers(&json))
}

async fn fetch_pscs(
    client: &reqwest::Client,
    api_key: &str,
    company_number: &str,
) -> Option<Vec<PscRecord>> {
    let resp = client
        .get(format!(
            "{BASE_URL}/company/{company_number}/persons-with-significant-control"
        ))
        .basic_auth(api_key, Option::<&str>::None)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    Some(parse_pscs(&json))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn company_search_fixture() -> serde_json::Value {
        serde_json::json!({
            "items": [
                {
                    "company_number": "12345678",
                    "title": "Acme Holdings Ltd",
                    "company_type": "ltd",
                    "company_status": "active",
                    "date_of_creation": "2015-03-12",
                    "address": {
                        "premises": "1",
                        "address_line_1": "High Street",
                        "locality": "London",
                        "postal_code": "EC1A 1BB",
                        "country": "England"
                    },
                    "sic_codes": ["64110", "64191"]
                },
                {
                    "company_number": "87654321",
                    "title": "Acme Services Ltd",
                    "company_type": "ltd",
                    "company_status": "dissolved",
                    "date_of_creation": "2010-07-01"
                }
            ],
            "total_results": 2
        })
    }

    fn officers_fixture() -> serde_json::Value {
        serde_json::json!({
            "items": [
                {
                    "name": "SMITH, John William",
                    "officer_role": "director",
                    "appointed_on": "2015-03-12",
                    "nationality": "British",
                    "occupation": "Company Director"
                },
                {
                    "name": "JONES, Alice",
                    "officer_role": "secretary",
                    "appointed_on": "2015-03-12",
                    "resigned_on": "2019-06-30"
                }
            ]
        })
    }

    fn psc_fixture() -> serde_json::Value {
        serde_json::json!({
            "items": [
                {
                    "name": "Smith, John William",
                    "natures_of_control": [
                        "ownership-of-shares-75-to-100-percent",
                        "voting-rights-75-to-100-percent"
                    ],
                    "notified_on": "2016-07-06",
                    "country_of_residence": "United Kingdom",
                    "date_of_birth": {
                        "month": 3,
                        "year": 1975
                    }
                }
            ]
        })
    }

    #[test]
    fn uk_ch_parses_company_search_result() {
        let json = company_search_fixture();
        let companies = parse_search_results(&json);

        assert_eq!(companies.len(), 2);
        assert_eq!(companies[0].company_number, "12345678");
        assert_eq!(companies[0].company_name, "Acme Holdings Ltd");
        assert_eq!(companies[0].company_status.as_deref(), Some("active"));
        assert_eq!(companies[0].date_of_creation.as_deref(), Some("2015-03-12"));
        assert_eq!(
            companies[0].registered_office_address.as_deref(),
            Some("1, High Street, London, EC1A 1BB, England")
        );
        assert_eq!(companies[0].sic_codes, vec!["64110", "64191"]);
        assert_eq!(companies[1].company_status.as_deref(), Some("dissolved"));
    }

    #[test]
    fn uk_ch_parses_officers_and_extracts_roles() {
        let json = officers_fixture();
        let officers = parse_officers(&json);

        assert_eq!(officers.len(), 2);
        assert_eq!(officers[0].name, "SMITH, John William");
        assert_eq!(officers[0].officer_role, "director");
        assert_eq!(officers[0].nationality.as_deref(), Some("British"));
        assert_eq!(officers[1].officer_role, "secretary");
        assert_eq!(officers[1].resigned_on.as_deref(), Some("2019-06-30"));
    }

    #[test]
    fn uk_ch_parses_pscs_with_natures_of_control() {
        let json = psc_fixture();
        let pscs = parse_pscs(&json);

        assert_eq!(pscs.len(), 1);
        assert_eq!(pscs[0].name, "Smith, John William");
        assert!(pscs[0]
            .natures_of_control
            .contains(&"ownership-of-shares-75-to-100-percent".to_string()));
        assert_eq!(pscs[0].notified_on.as_deref(), Some("2016-07-06"));
        assert_eq!(pscs[0].date_of_birth.as_deref(), Some("03/1975"));
    }

    #[test]
    fn uk_ch_handles_empty_results() {
        let json = serde_json::json!({ "items": [], "total_results": 0 });
        let companies = parse_search_results(&json);
        assert!(companies.is_empty());

        let officers = parse_officers(&serde_json::json!({ "items": [] }));
        assert!(officers.is_empty());

        let pscs = parse_pscs(&serde_json::json!({ "items": [] }));
        assert!(pscs.is_empty());
    }
}
