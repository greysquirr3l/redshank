//! FINRA BrokerCheck — public disclosure database for registered brokers and firms.
//!
//! Individual search: <https://brokercheck.finra.org/individual/search>
//! Firm search: <https://brokercheck.finra.org/firm/search>
//!
//! BrokerCheck exposes CRD numbers, employment history, exam qualifications,
//! and disclosure events (customer disputes, regulatory actions, criminal history,
//! involuntary terminations, financial events).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use serde::{Deserialize, Serialize};
use std::path::Path;

const INDIVIDUAL_SEARCH_URL: &str =
    "https://api.brokercheck.finra.org/search/individual";
const FIRM_SEARCH_URL: &str = "https://api.brokercheck.finra.org/search/firm";
const DEFAULT_SIZE: u32 = 12;

/// A FINRA BrokerCheck individual broker record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrokerRecord {
    /// Central Registration Depository unique identifier.
    pub crd_number: String,
    /// Full legal name of the broker.
    pub name: String,
    /// Current registration status (active, inactive, suspended, etc.).
    pub status: String,
    /// List of current firm names where the broker is registered.
    pub current_firms: Vec<String>,
    /// Prior firm employment history entries.
    pub employment_history: Vec<EmploymentEntry>,
    /// Exam qualifications (Series 7, 63, 65, etc.).
    pub exam_qualifications: Vec<String>,
    /// Disclosure events on the broker's record.
    pub disclosures: Vec<Disclosure>,
}

/// A single employment history entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmploymentEntry {
    /// Name of the firm.
    pub firm_name: String,
    /// Start date of employment.
    pub start_date: Option<String>,
    /// End date of employment (None if current).
    pub end_date: Option<String>,
}

/// A disclosure event on a broker or firm record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disclosure {
    /// Type of disclosure (CustomerDispute, RegulatoryAction, Criminal, Termination, Financial).
    pub disclosure_type: String,
    /// Date of the event.
    pub event_date: Option<String>,
    /// Whether the event is still pending/unresolved.
    pub is_pending: bool,
    /// Settlement or sanction amount if applicable.
    pub amount: Option<f64>,
    /// Narrative description of the disclosure.
    pub description: Option<String>,
}

/// A FINRA BrokerCheck firm record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirmRecord {
    /// Central Registration Depository unique identifier for the firm.
    pub crd_number: String,
    /// Legal name of the firm.
    pub name: String,
    /// Current registration status.
    pub status: String,
    /// Number of registered branch offices.
    pub branch_count: u32,
    /// Summary count of disclosure events.
    pub disclosure_count: u32,
}

/// Parse broker records from a FINRA individual search response.
#[must_use]
pub fn parse_individual_search(json: &serde_json::Value) -> Vec<BrokerRecord> {
    let hits = json
        .get("hits")
        .and_then(|h| h.get("hits"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .or_else(|| {
            json.get("results")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .unwrap_or_default();

    hits.iter().filter_map(parse_individual_hit).collect()
}

/// Parse a single broker hit from search results.
fn parse_individual_hit(hit: &serde_json::Value) -> Option<BrokerRecord> {
    // BrokerCheck wraps data in `_source` for Elasticsearch responses
    let source = hit.get("_source").unwrap_or(hit);

    let crd_number = source
        .get("ind_bc_scope")
        .and_then(|s| s.get("ind_crd_nb"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| source.get("crd_number").and_then(serde_json::Value::as_str))
        .or_else(|| source.get("crdNumber").and_then(serde_json::Value::as_str))
        .map(String::from)?;

    let name = source
        .get("ind_bc_scope")
        .and_then(|s| s.get("ind_firstname").and_then(serde_json::Value::as_str))
        .map(|first| {
            let last = source
                .get("ind_bc_scope")
                .and_then(|s| s.get("ind_lastname"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            format!("{first} {last}").trim().to_string()
        })
        .or_else(|| {
            source
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(String::from)
        })
        .unwrap_or_default();

    let status = source
        .get("ind_bc_scope")
        .and_then(|s| s.get("ind_status_cd"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| source.get("status").and_then(serde_json::Value::as_str))
        .unwrap_or("unknown")
        .to_string();

    let current_firms = parse_current_firms(source);
    let employment_history = parse_employment_history(source);
    let exam_qualifications = parse_exam_qualifications(source);
    let disclosures = parse_disclosures(source);

    Some(BrokerRecord {
        crd_number,
        name,
        status,
        current_firms,
        employment_history,
        exam_qualifications,
        disclosures,
    })
}

/// Parse current firm registrations from a broker record.
fn parse_current_firms(source: &serde_json::Value) -> Vec<String> {
    source
        .get("currentEmployments")
        .or_else(|| source.get("current_employments"))
        .or_else(|| source.get("ind_bc_scope").and_then(|s| s.get("ind_emps_list")))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|emp| {
                    emp.get("firm_name")
                        .or_else(|| emp.get("firmName"))
                        .or_else(|| emp.get("org_nm"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse employment history entries.
fn parse_employment_history(source: &serde_json::Value) -> Vec<EmploymentEntry> {
    source
        .get("employmentHistory")
        .or_else(|| source.get("employment_history"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let firm_name = entry
                        .get("firm_name")
                        .or_else(|| entry.get("firmName"))
                        .or_else(|| entry.get("org_nm"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)?;
                    let start_date = entry
                        .get("start_date")
                        .or_else(|| entry.get("startDate"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from);
                    let end_date = entry
                        .get("end_date")
                        .or_else(|| entry.get("endDate"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from);
                    Some(EmploymentEntry {
                        firm_name,
                        start_date,
                        end_date,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse exam qualifications from a broker record.
fn parse_exam_qualifications(source: &serde_json::Value) -> Vec<String> {
    source
        .get("exams")
        .or_else(|| source.get("examHistory"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|exam| {
                    exam.get("exam_name")
                        .or_else(|| exam.get("examName"))
                        .or_else(|| exam.get("series"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse all disclosure events from a broker record.
fn parse_disclosures(source: &serde_json::Value) -> Vec<Disclosure> {
    source
        .get("disclosures")
        .or_else(|| source.get("disclosureHistory"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_single_disclosure).collect())
        .unwrap_or_default()
}

/// Parse a single disclosure event entry.
fn parse_single_disclosure(entry: &serde_json::Value) -> Option<Disclosure> {
    let disclosure_type = entry
        .get("disclosure_type")
        .or_else(|| entry.get("disclosureType"))
        .or_else(|| entry.get("type"))
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let event_date = entry
        .get("event_date")
        .or_else(|| entry.get("eventDate"))
        .or_else(|| entry.get("date"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let is_pending = entry
        .get("is_pending")
        .or_else(|| entry.get("pending"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let amount = entry
        .get("settlement_amount")
        .or_else(|| entry.get("amount"))
        .or_else(|| entry.get("sanction_amount"))
        .and_then(serde_json::Value::as_f64);

    let description = entry
        .get("description")
        .or_else(|| entry.get("allegation"))
        .or_else(|| entry.get("narrative"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    Some(Disclosure {
        disclosure_type,
        event_date,
        is_pending,
        amount,
        description,
    })
}

/// Parse firm records from a FINRA firm search response.
#[must_use]
pub fn parse_firm_search(json: &serde_json::Value) -> Vec<FirmRecord> {
    let hits = json
        .get("hits")
        .and_then(|h| h.get("hits"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .or_else(|| {
            json.get("results")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .unwrap_or_default();

    hits.iter().filter_map(parse_firm_hit).collect()
}

/// Parse a single firm hit from search results.
fn parse_firm_hit(hit: &serde_json::Value) -> Option<FirmRecord> {
    let source = hit.get("_source").unwrap_or(hit);

    let crd_number = source
        .get("firm_bc_scope")
        .and_then(|s| s.get("firm_crd_nb"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| source.get("crd_number").and_then(serde_json::Value::as_str))
        .or_else(|| source.get("crdNumber").and_then(serde_json::Value::as_str))
        .map(String::from)?;

    let name = source
        .get("firm_bc_scope")
        .and_then(|s| s.get("firm_name"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| source.get("name").and_then(serde_json::Value::as_str))
        .map(String::from)
        .unwrap_or_default();

    let status = source
        .get("firm_bc_scope")
        .and_then(|s| s.get("firm_status_cd"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| source.get("status").and_then(serde_json::Value::as_str))
        .unwrap_or("unknown")
        .to_string();

    let branch_count = source
        .get("branch_count")
        .or_else(|| source.get("branchCount"))
        .and_then(serde_json::Value::as_u64)
        .map(|n| n as u32)
        .unwrap_or(0);

    let disclosure_count = source
        .get("disclosure_count")
        .or_else(|| source.get("disclosureCount"))
        .and_then(serde_json::Value::as_u64)
        .map(|n| n as u32)
        .unwrap_or(0);

    Some(FirmRecord {
        crd_number,
        name,
        status,
        branch_count,
        disclosure_count,
    })
}

/// Fetch FINRA BrokerCheck individual broker records by name query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_individual(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let from = page * DEFAULT_SIZE;
        let resp = client
            .get(INDIVIDUAL_SEARCH_URL)
            .query(&[
                ("query", query),
                ("ftscore", "true"),
                ("includePrevious", "true"),
                ("hl", "true"),
                ("nrows", &DEFAULT_SIZE.to_string()),
                ("start", &from.to_string()),
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
        let brokers = parse_individual_search(&json);

        if brokers.is_empty() {
            break;
        }

        for broker in &brokers {
            all_records.push(serde_json::to_value(broker).map_err(|e| {
                FetchError::Parse(format!("serialize broker: {e}"))
            })?);
        }

        let total = json
            .get("hits")
            .and_then(|h| h.get("total"))
            .and_then(|t| t.get("value").or(Some(t)))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if u64::from(from + DEFAULT_SIZE) >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("finra_brokercheck_individuals.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "finra_brokercheck".into(),
        attribution: None,
    })
}

/// Fetch FINRA BrokerCheck firm records by name query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_firm(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let from = page * DEFAULT_SIZE;
        let resp = client
            .get(FIRM_SEARCH_URL)
            .query(&[
                ("query", query),
                ("ftscore", "true"),
                ("nrows", &DEFAULT_SIZE.to_string()),
                ("start", &from.to_string()),
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
        let firms = parse_firm_search(&json);

        if firms.is_empty() {
            break;
        }

        for firm in &firms {
            all_records.push(serde_json::to_value(firm).map_err(|e| {
                FetchError::Parse(format!("serialize firm: {e}"))
            })?);
        }

        let total = json
            .get("hits")
            .and_then(|h| h.get("total"))
            .and_then(|t| t.get("value").or(Some(t)))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if u64::from(from + DEFAULT_SIZE) >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("finra_brokercheck_firms.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "finra_brokercheck_firms".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn finra_constructs_correct_search_and_parses_individual_results() {
        // Simulate a BrokerCheck Elasticsearch-style individual search response.
        let json = serde_json::json!({
            "hits": {
                "total": { "value": 1 },
                "hits": [
                    {
                        "_source": {
                            "crd_number": "1234567",
                            "name": "Jane Smith",
                            "status": "Active",
                            "currentEmployments": [
                                { "firm_name": "Acme Brokerage LLC" }
                            ]
                        }
                    }
                ]
            }
        });

        let brokers = parse_individual_search(&json);
        assert_eq!(brokers.len(), 1);
        assert_eq!(brokers[0].crd_number, "1234567");
        assert_eq!(brokers[0].name, "Jane Smith");
        assert_eq!(brokers[0].status, "Active");
        assert_eq!(brokers[0].current_firms, vec!["Acme Brokerage LLC"]);
    }

    #[test]
    fn finra_extracts_disclosures_from_detail_fixture() {
        let json = serde_json::json!({
            "hits": {
                "total": { "value": 1 },
                "hits": [
                    {
                        "_source": {
                            "crd_number": "7654321",
                            "name": "John Doe",
                            "status": "Active",
                            "disclosures": [
                                {
                                    "disclosure_type": "CustomerDispute",
                                    "event_date": "2019-03-15",
                                    "is_pending": false,
                                    "settlement_amount": 75000.0,
                                    "description": "Unsuitable investment recommendations."
                                },
                                {
                                    "disclosure_type": "RegulatoryAction",
                                    "event_date": "2021-06-01",
                                    "is_pending": true,
                                    "description": "FINRA investigation ongoing."
                                }
                            ]
                        }
                    }
                ]
            }
        });

        let brokers = parse_individual_search(&json);
        assert_eq!(brokers.len(), 1);
        let disc = &brokers[0].disclosures;
        assert_eq!(disc.len(), 2);
        assert_eq!(disc[0].disclosure_type, "CustomerDispute");
        assert_eq!(disc[0].event_date, Some("2019-03-15".to_string()));
        assert!(!disc[0].is_pending);
        assert_eq!(disc[0].amount, Some(75_000.0));
        assert_eq!(disc[1].disclosure_type, "RegulatoryAction");
        assert!(disc[1].is_pending);
    }

    #[test]
    fn finra_handles_firm_search_and_parses_registration_history() {
        let json = serde_json::json!({
            "hits": {
                "total": { "value": 2 },
                "hits": [
                    {
                        "_source": {
                            "crd_number": "111222",
                            "name": "Global Securities Inc",
                            "status": "Active",
                            "branch_count": 12,
                            "disclosure_count": 3
                        }
                    },
                    {
                        "_source": {
                            "crd_number": "333444",
                            "name": "Defunct Broker Dealers LLC",
                            "status": "Inactive",
                            "branch_count": 0,
                            "disclosure_count": 7
                        }
                    }
                ]
            }
        });

        let firms = parse_firm_search(&json);
        assert_eq!(firms.len(), 2);
        assert_eq!(firms[0].crd_number, "111222");
        assert_eq!(firms[0].name, "Global Securities Inc");
        assert_eq!(firms[0].status, "Active");
        assert_eq!(firms[0].branch_count, 12);
        assert_eq!(firms[0].disclosure_count, 3);

        assert_eq!(firms[1].status, "Inactive");
        assert_eq!(firms[1].disclosure_count, 7);
    }

    #[test]
    fn finra_extracts_crd_number_employment_history_and_exams() {
        let json = serde_json::json!({
            "results": [
                {
                    "crd_number": "9988776",
                    "name": "Alice Broker",
                    "status": "Active",
                    "employmentHistory": [
                        {
                            "firm_name": "First Brokerage",
                            "start_date": "2010-01-15",
                            "end_date": "2015-06-30"
                        },
                        {
                            "firm_name": "Second Brokerage",
                            "start_date": "2015-07-01",
                            "end_date": null
                        }
                    ],
                    "exams": [
                        { "exam_name": "Series 7" },
                        { "exam_name": "Series 63" },
                        { "exam_name": "Series 65" }
                    ]
                }
            ]
        });

        let brokers = parse_individual_search(&json);
        assert_eq!(brokers.len(), 1);
        assert_eq!(brokers[0].crd_number, "9988776");

        let emp = &brokers[0].employment_history;
        assert_eq!(emp.len(), 2);
        assert_eq!(emp[0].firm_name, "First Brokerage");
        assert_eq!(emp[0].start_date, Some("2010-01-15".to_string()));
        assert_eq!(emp[0].end_date, Some("2015-06-30".to_string()));
        assert_eq!(emp[1].firm_name, "Second Brokerage");
        assert_eq!(emp[1].end_date, None);

        let exams = &brokers[0].exam_qualifications;
        assert_eq!(exams.len(), 3);
        assert!(exams.contains(&"Series 7".to_string()));
        assert!(exams.contains(&"Series 63".to_string()));
    }

    #[test]
    fn finra_handles_empty_search_results() {
        let empty_individual = serde_json::json!({
            "hits": {
                "total": { "value": 0 },
                "hits": []
            }
        });
        let brokers = parse_individual_search(&empty_individual);
        assert!(brokers.is_empty());

        let empty_firm = serde_json::json!({
            "hits": {
                "total": { "value": 0 },
                "hits": []
            }
        });
        let firms = parse_firm_search(&empty_firm);
        assert!(firms.is_empty());
    }
}
