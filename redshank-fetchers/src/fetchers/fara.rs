//! FARA — Foreign Agents Registration Act database (DOJ National Security Division).
//!
//! Bulk XML: <https://efile.fara.gov/ords/fara/f?p=API:BULKDATA>
//! Search API: <https://efile.fara.gov/ords/fara/f?p=API:SEARCH>
//!
//! FARA registrations are mandatory when representing foreign governments or
//! political parties to influence US policy. Non-registration is a federal crime.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use serde::{Deserialize, Serialize};
use std::path::Path;

const SEARCH_API_BASE: &str = "https://efile.fara.gov/api/v1/Registrants/search";

/// A FARA registrant record parsed from the API or XML response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaraRegistrant {
    /// Unique registration number.
    pub registration_number: String,
    /// Name of the registrant (typically a lobbying firm or individual).
    pub registrant_name: String,
    /// Foreign principal being represented (country or entity).
    pub foreign_principal: String,
    /// Date of registration.
    pub registration_date: Option<String>,
    /// Termination date (if registration is terminated).
    pub termination_date: Option<String>,
    /// Whether the registration is currently active.
    pub is_active: bool,
    /// Primary activities performed (lobbying, public relations, etc.).
    pub activities: Vec<String>,
    /// Total compensation reported.
    pub compensation: Option<String>,
}

/// A supplemental statement record from a registrant's semi-annual report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaraSupplementalStatement {
    /// Registration number this statement belongs to.
    pub registration_number: String,
    /// Reporting period start date.
    pub period_start: Option<String>,
    /// Reporting period end date.
    pub period_end: Option<String>,
    /// Detailed activity descriptions.
    pub activities: Vec<String>,
    /// Disbursement breakdown by category.
    pub disbursements: Vec<FaraDisbursement>,
}

/// A single disbursement entry from a supplemental statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaraDisbursement {
    /// Category of disbursement (e.g., "media placements", "travel").
    pub category: String,
    /// Amount in USD.
    pub amount: Option<String>,
    /// Description of the disbursement.
    pub description: Option<String>,
}

/// Parse registrant records from FARA API JSON response.
#[must_use]
pub fn parse_registrants_json(json: &serde_json::Value) -> Vec<FaraRegistrant> {
    extract_registrant_array(json)
        .iter()
        .filter_map(parse_single_registrant)
        .collect()
}

/// Extract the registrant array from various response formats.
fn extract_registrant_array(json: &serde_json::Value) -> Vec<serde_json::Value> {
    // Try "REGISTRANTS" key first, then "registrants", then assume top-level array
    json.get("REGISTRANTS")
        .or_else(|| json.get("registrants"))
        .or_else(|| json.get("items"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .or_else(|| json.as_array().cloned())
        .unwrap_or_default()
}

/// Parse a single registrant from a JSON object.
fn parse_single_registrant(item: &serde_json::Value) -> Option<FaraRegistrant> {
    let reg_num_field = item
        .get("Registration_Number")
        .or_else(|| item.get("registration_number"))
        .or_else(|| item.get("regNum"));

    let reg_num: String = reg_num_field
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .or_else(|| {
            reg_num_field
                .and_then(serde_json::Value::as_u64)
                .map(|n| n.to_string())
        })?;

    let registrant_name = item
        .get("Registrant_Name")
        .or_else(|| item.get("registrant_name"))
        .or_else(|| item.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    let foreign_principal = item
        .get("Foreign_Principal")
        .or_else(|| item.get("foreign_principal"))
        .or_else(|| item.get("foreignPrincipal"))
        .or_else(|| item.get("FP_Name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();

    let registration_date = item
        .get("Registration_Date")
        .or_else(|| item.get("registration_date"))
        .or_else(|| item.get("regDate"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let termination_date = item
        .get("Termination_Date")
        .or_else(|| item.get("termination_date"))
        .or_else(|| item.get("termDate"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let is_active = termination_date.is_none()
        || item
            .get("Status")
            .or_else(|| item.get("status"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|s| s.eq_ignore_ascii_case("active"));

    let activities = item
        .get("Activities")
        .or_else(|| item.get("activities"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .or_else(|| {
            item.get("Activities")
                .or_else(|| item.get("activities"))
                .and_then(serde_json::Value::as_str)
                .map(|s| vec![s.to_string()])
        })
        .unwrap_or_default();

    let compensation = item
        .get("Compensation")
        .or_else(|| item.get("compensation"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    Some(FaraRegistrant {
        registration_number: reg_num,
        registrant_name,
        foreign_principal,
        registration_date,
        termination_date,
        is_active,
        activities,
        compensation,
    })
}

/// Parse supplemental statements from FARA response.
#[must_use]
pub fn parse_supplemental_statements(json: &serde_json::Value) -> Vec<FaraSupplementalStatement> {
    let items = json
        .get("SUPPLEMENTAL_STATEMENTS")
        .or_else(|| json.get("supplemental_statements"))
        .or_else(|| json.get("statements"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    items
        .iter()
        .filter_map(|item| {
            let reg_num = item
                .get("Registration_Number")
                .or_else(|| item.get("registration_number"))
                .and_then(|v| v.as_str().map(String::from).or_else(|| v.as_u64().map(|n| n.to_string())))?;

            let period_start = item
                .get("Period_Start")
                .or_else(|| item.get("period_start"))
                .and_then(serde_json::Value::as_str)
                .map(String::from);

            let period_end = item
                .get("Period_End")
                .or_else(|| item.get("period_end"))
                .and_then(serde_json::Value::as_str)
                .map(String::from);

            let activities = item
                .get("Activities")
                .or_else(|| item.get("activities"))
                .and_then(serde_json::Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let disbursements = item
                .get("Disbursements")
                .or_else(|| item.get("disbursements"))
                .and_then(serde_json::Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|d| {
                            let category = d
                                .get("Category")
                                .or_else(|| d.get("category"))
                                .and_then(serde_json::Value::as_str)?
                                .to_string();
                            let amount = d
                                .get("Amount")
                                .or_else(|| d.get("amount"))
                                .and_then(serde_json::Value::as_str)
                                .map(String::from);
                            let description = d
                                .get("Description")
                                .or_else(|| d.get("description"))
                                .and_then(serde_json::Value::as_str)
                                .map(String::from);
                            Some(FaraDisbursement {
                                category,
                                amount,
                                description,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            Some(FaraSupplementalStatement {
                registration_number: reg_num,
                period_start,
                period_end,
                activities,
                disbursements,
            })
        })
        .collect()
}

/// Parse FARA registrant data from bulk XML content.
#[must_use]
pub fn parse_registrants_xml(xml_content: &str) -> Vec<FaraRegistrant> {
    let mut records = Vec::new();
    let mut pos = 0;

    while let Some(start) = xml_content.get(pos..).and_then(|s| s.find("<Registrant>")) {
        let abs_start = pos + start;
        if let Some(end) = xml_content.get(abs_start..).and_then(|s| s.find("</Registrant>")) {
            let block_end = abs_start + end + "</Registrant>".len();
            let block = xml_content.get(abs_start..block_end).unwrap_or("");
            
            let reg_num = extract_xml_tag(block, "Registration_Number");
            if reg_num.is_empty() {
                pos = block_end;
                continue;
            }

            let registrant_name = extract_xml_tag(block, "Registrant_Name");
            let foreign_principal = extract_xml_tag(block, "Foreign_Principal");
            let registration_date = extract_xml_tag_option(block, "Registration_Date");
            let termination_date = extract_xml_tag_option(block, "Termination_Date");
            let activities_str = extract_xml_tag(block, "Activities");
            let compensation = extract_xml_tag_option(block, "Compensation");

            let is_active = termination_date.is_none();
            let activities = if activities_str.is_empty() {
                Vec::new()
            } else {
                activities_str
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };

            records.push(FaraRegistrant {
                registration_number: reg_num,
                registrant_name,
                foreign_principal,
                registration_date,
                termination_date,
                is_active,
                activities,
                compensation,
            });
            pos = block_end;
        } else {
            break;
        }
    }
    records
}

/// Extract text content from an XML tag.
fn extract_xml_tag(text: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(start) = text.find(&open) {
        let after = start + open.len();
        if let Some(end_pos) = text.get(after..).and_then(|s| s.find(&close)) {
            return text.get(after..after + end_pos).unwrap_or("").to_string();
        }
    }
    String::new()
}

/// Extract optional text content from an XML tag.
fn extract_xml_tag_option(text: &str, tag: &str) -> Option<String> {
    let val = extract_xml_tag(text, tag);
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

/// Fetch FARA registrations matching the search query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_registrations(
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
            .get(SEARCH_API_BASE)
            .query(&[
                ("search", query),
                ("page", &page.to_string()),
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
        let registrants = parse_registrants_json(&json);

        if registrants.is_empty() {
            break;
        }

        // Convert to JSON values for NDJSON output
        for reg in registrants {
            all_records.push(serde_json::to_value(&reg).map_err(|e| {
                FetchError::Parse(format!("serialize registrant: {e}"))
            })?);
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("fara_registrations.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "fara".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn fara_parses_registrant_json_extracts_name_principal_and_date() {
        let json = serde_json::json!({
            "REGISTRANTS": [
                {
                    "Registration_Number": "6789",
                    "Registrant_Name": "Podesta Group Inc",
                    "Foreign_Principal": "Government of Ukraine",
                    "Registration_Date": "2012-06-01",
                    "Status": "Active",
                    "Activities": ["Lobbying", "Public Relations"],
                    "Compensation": "$1,200,000"
                }
            ]
        });

        let registrants = parse_registrants_json(&json);
        assert_eq!(registrants.len(), 1);
        assert_eq!(registrants[0].registration_number, "6789");
        assert_eq!(registrants[0].registrant_name, "Podesta Group Inc");
        assert_eq!(registrants[0].foreign_principal, "Government of Ukraine");
        assert_eq!(registrants[0].registration_date, Some("2012-06-01".to_string()));
        assert!(registrants[0].is_active);
        assert_eq!(registrants[0].activities.len(), 2);
        assert!(registrants[0].activities.contains(&"Lobbying".to_string()));
    }

    #[test]
    fn fara_extracts_activities_and_compensation_from_supplemental() {
        let json = serde_json::json!({
            "SUPPLEMENTAL_STATEMENTS": [
                {
                    "Registration_Number": "6789",
                    "Period_Start": "2022-01-01",
                    "Period_End": "2022-06-30",
                    "Activities": [
                        "Congressional outreach",
                        "Media placement coordination"
                    ],
                    "Disbursements": [
                        {
                            "Category": "Travel",
                            "Amount": "$15,000",
                            "Description": "Congressional delegation trips"
                        },
                        {
                            "Category": "Media",
                            "Amount": "$50,000",
                            "Description": "Op-ed placement and PR"
                        }
                    ]
                }
            ]
        });

        let statements = parse_supplemental_statements(&json);
        assert_eq!(statements.len(), 1);
        assert_eq!(statements[0].registration_number, "6789");
        assert_eq!(statements[0].period_start, Some("2022-01-01".to_string()));
        assert_eq!(statements[0].period_end, Some("2022-06-30".to_string()));
        assert_eq!(statements[0].activities.len(), 2);
        assert!(statements[0].activities.contains(&"Congressional outreach".to_string()));
        assert_eq!(statements[0].disbursements.len(), 2);
        assert_eq!(statements[0].disbursements[0].category, "Travel");
        assert_eq!(statements[0].disbursements[0].amount, Some("$15,000".to_string()));
    }

    #[test]
    fn fara_handles_active_and_terminated_registrations() {
        let json = serde_json::json!({
            "REGISTRANTS": [
                {
                    "Registration_Number": "1001",
                    "Registrant_Name": "Active Lobbyist LLC",
                    "Foreign_Principal": "Country A",
                    "Registration_Date": "2020-01-15",
                    "Status": "Active"
                },
                {
                    "Registration_Number": "1002",
                    "Registrant_Name": "Former Agent Corp",
                    "Foreign_Principal": "Country B",
                    "Registration_Date": "2015-03-01",
                    "Termination_Date": "2021-12-31",
                    "Status": "Terminated"
                }
            ]
        });

        let registrants = parse_registrants_json(&json);
        assert_eq!(registrants.len(), 2);

        // Active registration
        assert!(registrants[0].is_active);
        assert!(registrants[0].termination_date.is_none());

        // Terminated registration
        assert!(!registrants[1].is_active);
        assert_eq!(registrants[1].termination_date, Some("2021-12-31".to_string()));
    }

    #[test]
    fn fara_parses_xml_registrant_format() {
        let xml = r#"<Registrants>
            <Registrant>
                <Registration_Number>5555</Registration_Number>
                <Registrant_Name>Mercury Public Affairs</Registrant_Name>
                <Foreign_Principal>Embassy of Kazakhstan</Foreign_Principal>
                <Registration_Date>2018-04-10</Registration_Date>
                <Activities>Lobbying; Political consulting</Activities>
                <Compensation>$2,500,000</Compensation>
            </Registrant>
            <Registrant>
                <Registration_Number>5556</Registration_Number>
                <Registrant_Name>Terminated Firm Inc</Registrant_Name>
                <Foreign_Principal>Country X</Foreign_Principal>
                <Registration_Date>2010-01-01</Registration_Date>
                <Termination_Date>2019-06-30</Termination_Date>
            </Registrant>
        </Registrants>"#;

        let registrants = parse_registrants_xml(xml);
        assert_eq!(registrants.len(), 2);
        assert_eq!(registrants[0].registration_number, "5555");
        assert_eq!(registrants[0].registrant_name, "Mercury Public Affairs");
        assert_eq!(registrants[0].foreign_principal, "Embassy of Kazakhstan");
        assert_eq!(registrants[0].compensation, Some("$2,500,000".to_string()));
        assert!(registrants[0].is_active);
        assert_eq!(registrants[0].activities.len(), 2);
        assert!(registrants[0].activities.contains(&"Lobbying".to_string()));
        assert!(registrants[0].activities.contains(&"Political consulting".to_string()));

        // Terminated registration
        assert!(!registrants[1].is_active);
        assert_eq!(registrants[1].termination_date, Some("2019-06-30".to_string()));
    }

    #[test]
    fn fara_handles_empty_responses() {
        let empty_json = serde_json::json!({
            "REGISTRANTS": []
        });
        let registrants = parse_registrants_json(&empty_json);
        assert!(registrants.is_empty());

        let empty_statements = serde_json::json!({
            "SUPPLEMENTAL_STATEMENTS": []
        });
        let statements = parse_supplemental_statements(&empty_statements);
        assert!(statements.is_empty());

        let empty_xml = "<Registrants></Registrants>";
        let xml_registrants = parse_registrants_xml(empty_xml);
        assert!(xml_registrants.is_empty());
    }
}
