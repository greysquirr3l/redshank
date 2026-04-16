//! Unified UK corporate intelligence fetcher built from `Companies House` and `OpenCorporates`.

use crate::domain::{Attribution, FetchError, FetchOutput};
use crate::fetchers::opencorporates;
use crate::fetchers::uk_companies_house::{CompanyRecord, OfficerRecord, PscRecord};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::collections::BTreeMap;
use std::path::Path;

const COMPANIES_HOUSE_BASE_URL: &str = "https://api.company-information.service.gov.uk";
const OPENCORPORATES_BASE_URL: &str = "https://api.opencorporates.com/v0.4";

/// Normalized `OpenCorporates` fields for a UK company.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenCorporatesUkRecord {
    /// Registry company number.
    pub company_number: String,
    /// Company name.
    pub company_name: String,
    /// Current registry status.
    pub current_status: Option<String>,
    /// Registered address as returned by `OpenCorporates`.
    pub registered_address: Option<String>,
    /// Canonical `OpenCorporates` entity URL.
    pub opencorporates_url: Option<String>,
    /// `OpenCorporates` officer records.
    pub officers: Vec<OfficerRecord>,
}

/// Unified UK company intelligence record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnifiedUkCompanyRecord {
    /// Registry company number.
    pub company_number: String,
    /// Company name.
    pub company_name: String,
    /// Company type or legal form.
    pub company_type: Option<String>,
    /// Companies House status.
    pub companies_house_status: Option<String>,
    /// `OpenCorporates` status.
    pub opencorporates_status: Option<String>,
    /// Incorporation date if known.
    pub date_of_creation: Option<String>,
    /// Registered office or address.
    pub registered_office_address: Option<String>,
    /// SIC codes from Companies House.
    pub sic_codes: Vec<String>,
    /// Officers from Companies House.
    pub officers: Vec<OfficerRecord>,
    /// Officers surfaced by `OpenCorporates`.
    pub opencorporates_officers: Vec<OfficerRecord>,
    /// Persons with Significant Control from Companies House.
    pub pscs: Vec<PscRecord>,
    /// `OpenCorporates` entity URL when available.
    pub opencorporates_url: Option<String>,
    /// Names of the registries that contributed data.
    pub registry_sources: Vec<String>,
}

fn normalize_company_number(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
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

    let sic_codes = item
        .get("sic_codes")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    Some(CompanyRecord {
        company_number,
        company_name,
        company_type: item
            .get("company_type")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        company_status: item
            .get("company_status")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        date_of_creation: item
            .get("date_of_creation")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        registered_office_address: parse_address(
            item.get("address")
                .or_else(|| item.get("registered_office_address")),
        ),
        sic_codes,
        officers: Vec::new(),
        pscs: Vec::new(),
    })
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
            .map(ToString::to_string),
        resigned_on: item
            .get("resigned_on")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        nationality: item
            .get("nationality")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        occupation: item
            .get("occupation")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
    }
}

fn parse_psc_item(item: &serde_json::Value) -> PscRecord {
    let natures_of_control = item
        .get("natures_of_control")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    let date_of_birth = item.get("date_of_birth").map(|dob| {
        let month = dob
            .get("month")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let year = dob
            .get("year")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
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
            .map(ToString::to_string),
        ceased_on: item
            .get("ceased_on")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        country_of_residence: item
            .get("country_of_residence")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        date_of_birth,
    }
}

/// Parse Companies House search results into normalized company records.
#[must_use]
pub fn parse_companies_house_search_results(json: &serde_json::Value) -> Vec<CompanyRecord> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().filter_map(parse_company_item).collect())
        .unwrap_or_default()
}

fn parse_companies_house_officers(json: &serde_json::Value) -> Vec<OfficerRecord> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().map(parse_officer_item).collect())
        .unwrap_or_default()
}

fn parse_companies_house_pscs(json: &serde_json::Value) -> Vec<PscRecord> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().map(parse_psc_item).collect())
        .unwrap_or_default()
}

/// Parse `OpenCorporates` UK search results into normalized records.
#[must_use]
pub fn parse_opencorporates_uk_results(json: &serde_json::Value) -> Vec<OpenCorporatesUkRecord> {
    json.get("results")
        .and_then(|results| results.get("companies"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let company = item.get("company")?;
                    if company
                        .get("jurisdiction_code")
                        .and_then(serde_json::Value::as_str)
                        != Some("gb")
                    {
                        return None;
                    }

                    Some(OpenCorporatesUkRecord {
                        company_number: company
                            .get("company_number")
                            .and_then(serde_json::Value::as_str)?
                            .to_string(),
                        company_name: company
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        current_status: company
                            .get("current_status")
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string),
                        registered_address: company
                            .get("registered_address_in_full")
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string),
                        opencorporates_url: company
                            .get("opencorporates_url")
                            .or_else(|| company.get("registry_url"))
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string),
                        officers: Vec::new(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_opencorporates_officers(json: &serde_json::Value) -> Vec<OfficerRecord> {
    json.get("results")
        .and_then(|results| results.get("company"))
        .and_then(|company| company.get("officers"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let officer = item.get("officer")?;
                    Some(OfficerRecord {
                        name: officer
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        officer_role: officer
                            .get("position")
                            .or_else(|| officer.get("occupation"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                        appointed_on: officer
                            .get("start_date")
                            .or_else(|| officer.get("appointed_on"))
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string),
                        resigned_on: officer
                            .get("end_date")
                            .or_else(|| officer.get("resigned_on"))
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string),
                        nationality: officer
                            .get("nationality")
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string),
                        occupation: officer
                            .get("occupation")
                            .and_then(serde_json::Value::as_str)
                            .map(ToString::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_opencorporates_canonical_url(json: &serde_json::Value) -> Option<String> {
    json.get("results")
        .and_then(|results| results.get("company"))
        .and_then(|company| {
            company
                .get("opencorporates_url")
                .or_else(|| company.get("registry_url"))
        })
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

/// Merge `Companies House` and `OpenCorporates` UK records by company number.
#[must_use]
pub fn merge_uk_company_records(
    companies_house_records: &[CompanyRecord],
    opencorporates_records: &[OpenCorporatesUkRecord],
) -> Vec<UnifiedUkCompanyRecord> {
    let mut merged = BTreeMap::<String, UnifiedUkCompanyRecord>::new();

    for company in companies_house_records {
        let company_number = normalize_company_number(&company.company_number);
        merged.insert(
            company_number.clone(),
            UnifiedUkCompanyRecord {
                company_number,
                company_name: company.company_name.clone(),
                company_type: company.company_type.clone(),
                companies_house_status: company.company_status.clone(),
                opencorporates_status: None,
                date_of_creation: company.date_of_creation.clone(),
                registered_office_address: company.registered_office_address.clone(),
                sic_codes: company.sic_codes.clone(),
                officers: company.officers.clone(),
                opencorporates_officers: Vec::new(),
                pscs: company.pscs.clone(),
                opencorporates_url: None,
                registry_sources: vec!["companies_house".to_string()],
            },
        );
    }

    for company in opencorporates_records {
        let company_number = normalize_company_number(&company.company_number);
        let entry =
            merged
                .entry(company_number.clone())
                .or_insert_with(|| UnifiedUkCompanyRecord {
                    company_number,
                    company_name: company.company_name.clone(),
                    company_type: None,
                    companies_house_status: None,
                    opencorporates_status: None,
                    date_of_creation: None,
                    registered_office_address: company.registered_address.clone(),
                    sic_codes: Vec::new(),
                    officers: Vec::new(),
                    opencorporates_officers: Vec::new(),
                    pscs: Vec::new(),
                    opencorporates_url: None,
                    registry_sources: Vec::new(),
                });

        if entry.company_name.is_empty() {
            entry.company_name.clone_from(&company.company_name);
        }
        if entry.registered_office_address.is_none() {
            entry
                .registered_office_address
                .clone_from(&company.registered_address);
        }
        entry
            .opencorporates_status
            .clone_from(&company.current_status);
        entry
            .opencorporates_url
            .clone_from(&company.opencorporates_url);
        entry.opencorporates_officers.clone_from(&company.officers);
        if !entry
            .registry_sources
            .iter()
            .any(|source| source == "opencorporates")
        {
            entry.registry_sources.push("opencorporates".to_string());
        }
    }

    merged.into_values().collect()
}

fn unified_attribution(records: &[OpenCorporatesUkRecord]) -> Option<Attribution> {
    if records.is_empty() {
        None
    } else {
        Some(opencorporates::attribution())
    }
}

async fn fetch_companies_house_officers(
    client: &reqwest::Client,
    api_key: &str,
    company_number: &str,
    rate_limit_ms: u64,
) -> Option<Vec<OfficerRecord>> {
    rate_limit_delay(rate_limit_ms).await;
    let response = client
        .get(format!(
            "{COMPANIES_HOUSE_BASE_URL}/company/{company_number}/officers"
        ))
        .basic_auth(api_key, Option::<&str>::None)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: serde_json::Value = response.json().await.ok()?;
    Some(parse_companies_house_officers(&json))
}

async fn fetch_companies_house_pscs(
    client: &reqwest::Client,
    api_key: &str,
    company_number: &str,
) -> Option<Vec<PscRecord>> {
    let response = client
        .get(format!(
            "{COMPANIES_HOUSE_BASE_URL}/company/{company_number}/persons-with-significant-control"
        ))
        .basic_auth(api_key, Option::<&str>::None)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let json: serde_json::Value = response.json().await.ok()?;
    Some(parse_companies_house_pscs(&json))
}

async fn fetch_opencorporates_company_detail(
    client: &reqwest::Client,
    company_number: &str,
    api_token: Option<&str>,
) -> Result<Option<serde_json::Value>, FetchError> {
    let mut request = client
        .get(format!(
            "{OPENCORPORATES_BASE_URL}/companies/gb/{company_number}"
        ))
        .query(&[("normalise_company_name", "true")]);

    if let Some(token) = api_token {
        request = request.query(&[("api_token", token)]);
    }

    let response = request.send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    Ok(Some(response.json().await?))
}

/// Fetch and merge UK `Companies House` and `OpenCorporates` results.
///
/// # Errors
///
/// Returns `Err` if the `Companies House` request fails, the `OpenCorporates` request
/// fails, or the merged records cannot be written.
#[allow(clippy::too_many_lines)]
pub async fn fetch_uk_corporate_intelligence(
    query: &str,
    companies_house_api_key: &str,
    opencorporates_api_token: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_results: u32,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let items_per_page = max_results.min(100);
    let companies_house_response = client
        .get(format!("{COMPANIES_HOUSE_BASE_URL}/search/companies"))
        .basic_auth(companies_house_api_key, Option::<&str>::None)
        .query(&[
            ("q", query),
            ("items_per_page", &items_per_page.to_string()),
        ])
        .send()
        .await?;
    let companies_house_status = companies_house_response.status();
    if !companies_house_status.is_success() {
        let body = companies_house_response.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: companies_house_status.as_u16(),
            body,
        });
    }

    let companies_house_json: serde_json::Value = companies_house_response.json().await?;
    let mut companies_house_records = parse_companies_house_search_results(&companies_house_json);
    for company in &mut companies_house_records {
        let company_number = company.company_number.clone();
        company.officers = fetch_companies_house_officers(
            &client,
            companies_house_api_key,
            &company_number,
            rate_limit_ms,
        )
        .await
        .unwrap_or_default();
        rate_limit_delay(rate_limit_ms).await;
        company.pscs =
            fetch_companies_house_pscs(&client, companies_house_api_key, &company_number)
                .await
                .unwrap_or_default();
    }

    let max = if max_pages == 0 { u32::MAX } else { max_pages };
    let mut opencorporates_records = Vec::new();
    for page in 1..=max {
        let mut request = client
            .get(format!("{OPENCORPORATES_BASE_URL}/companies/search"))
            .query(&[
                ("q", query),
                ("jurisdiction_code", "gb"),
                ("page", &page.to_string()),
                ("per_page", "100"),
            ]);

        if let Some(token) = opencorporates_api_token {
            request = request.query(&[("api_token", token)]);
        }

        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(FetchError::ApiError {
                status: status.as_u16(),
                body,
            });
        }

        let json: serde_json::Value = response.json().await?;
        let page_records = parse_opencorporates_uk_results(&json);
        if page_records.is_empty() {
            break;
        }
        opencorporates_records.extend(page_records);

        let total_pages = u32::try_from(
            json.get("results")
                .and_then(|results| results.get("total_pages"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1),
        )
        .unwrap_or(u32::MAX);
        if page >= total_pages {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    for record in &mut opencorporates_records {
        if let Some(json) = fetch_opencorporates_company_detail(
            &client,
            &record.company_number,
            opencorporates_api_token,
        )
        .await?
        {
            record.officers = parse_opencorporates_officers(&json);
            if let Some(url) = parse_opencorporates_canonical_url(&json) {
                record.opencorporates_url = Some(url);
            }
            rate_limit_delay(rate_limit_ms).await;
        }
    }

    let merged_records =
        merge_uk_company_records(&companies_house_records, &opencorporates_records);
    let output_path = output_dir.join("uk_corporate_intelligence.ndjson");
    let values = merged_records
        .into_iter()
        .map(|record| {
            serde_json::to_value(record).map_err(|err| FetchError::Parse(err.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let count = write_ndjson(&output_path, &values)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "uk_corporate_intelligence".into(),
        attribution: unified_attribution(&opencorporates_records),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn companies_house_fixture() -> serde_json::Value {
        serde_json::json!({
            "items": [
                {
                    "company_number": "01234567",
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
                }
            ]
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
                }
            ]
        })
    }

    fn psc_fixture() -> serde_json::Value {
        serde_json::json!({
            "items": [
                {
                    "name": "Acme Family Trust",
                    "natures_of_control": ["ownership-of-shares-75-to-100-percent"],
                    "notified_on": "2016-04-06",
                    "country_of_residence": "United Kingdom",
                    "date_of_birth": {"month": 5, "year": 1980}
                }
            ]
        })
    }

    fn opencorporates_fixture() -> serde_json::Value {
        serde_json::json!({
            "results": {
                "companies": [
                    {
                        "company": {
                            "company_number": "01234567",
                            "name": "ACME HOLDINGS LTD",
                            "jurisdiction_code": "gb",
                            "registered_address_in_full": "1 High Street, London, EC1A 1BB, England",
                            "current_status": "Active",
                            "opencorporates_url": "https://opencorporates.com/companies/gb/01234567"
                        }
                    }
                ],
                "total_pages": 1
            }
        })
    }

    fn opencorporates_detail_fixture() -> serde_json::Value {
        serde_json::json!({
            "results": {
                "company": {
                    "company_number": "01234567",
                    "opencorporates_url": "https://opencorporates.com/companies/gb/01234567",
                    "officers": [
                        {
                            "officer": {
                                "name": "DOE, Jane",
                                "position": "director",
                                "start_date": "2017-01-01",
                                "nationality": "British",
                                "occupation": "Executive"
                            }
                        }
                    ]
                }
            }
        })
    }

    #[test]
    fn uk_corporate_intelligence_merges_companies_house_and_opencorporates_records() {
        let mut companies_house_records =
            parse_companies_house_search_results(&companies_house_fixture());
        companies_house_records[0].officers = parse_companies_house_officers(&officers_fixture());
        companies_house_records[0].pscs = parse_companies_house_pscs(&psc_fixture());
        let mut opencorporates_records = parse_opencorporates_uk_results(&opencorporates_fixture());
        opencorporates_records[0].officers =
            parse_opencorporates_officers(&opencorporates_detail_fixture());

        let merged = merge_uk_company_records(&companies_house_records, &opencorporates_records);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].company_number, "01234567");
        assert_eq!(merged[0].company_name, "Acme Holdings Ltd");
        assert_eq!(merged[0].officers.len(), 1);
        assert_eq!(merged[0].opencorporates_officers.len(), 1);
        assert_eq!(merged[0].pscs.len(), 1);
        assert_eq!(merged[0].opencorporates_status.as_deref(), Some("Active"));
        assert_eq!(
            merged[0].opencorporates_url.as_deref(),
            Some("https://opencorporates.com/companies/gb/01234567")
        );
        assert_eq!(
            merged[0].registry_sources,
            vec!["companies_house", "opencorporates"]
        );
    }

    #[test]
    fn uk_corporate_intelligence_parses_opencorporates_detail_officers_and_url() {
        let detail = opencorporates_detail_fixture();
        let officers = parse_opencorporates_officers(&detail);
        let canonical_url = parse_opencorporates_canonical_url(&detail);

        assert_eq!(officers.len(), 1);
        assert_eq!(officers[0].name, "DOE, Jane");
        assert_eq!(officers[0].officer_role, "director");
        assert_eq!(officers[0].appointed_on.as_deref(), Some("2017-01-01"));
        assert_eq!(
            canonical_url.as_deref(),
            Some("https://opencorporates.com/companies/gb/01234567")
        );
    }

    #[test]
    fn uk_corporate_intelligence_sets_opencorporates_attribution_when_present() {
        let opencorporates_records = parse_opencorporates_uk_results(&opencorporates_fixture());
        let attribution = unified_attribution(&opencorporates_records).unwrap();

        assert_eq!(attribution.source, "OpenCorporates");
        assert_eq!(attribution.text, "from OpenCorporates");
        assert_eq!(attribution.licence, "ODbL-1.0");
    }
}
