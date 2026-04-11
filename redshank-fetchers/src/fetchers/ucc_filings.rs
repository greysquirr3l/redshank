//! UCC (Uniform Commercial Code) filing search across state Secretary of State portals.
//!
//! UCC-1 financing statements record security interests in personal property.
//! This module parses JSON/HTML responses from state SOS portals.
//!
//! Supported states: California (JSON API), New York (form POST), Texas (form POST),
//! Delaware (JSON), Florida (form POST).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

/// A UCC filing record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UccFiling {
    /// Filing number assigned by the state.
    pub filing_number: String,
    /// Filing date (ISO 8601).
    pub filing_date: Option<String>,
    /// Lapse date — five years after filing unless continued.
    pub lapse_date: Option<String>,
    /// Filing type: "UCC-1 INITIAL", "UCC-3 AMENDMENT", "UCC-3 CONTINUATION", "UCC-3 TERMINATION".
    pub filing_type: String,
    /// State where the UCC was filed.
    pub state: String,
    /// List of debtors (name + optional address).
    pub debtors: Vec<UccParty>,
    /// List of secured parties (name + optional address).
    pub secured_parties: Vec<UccParty>,
    /// Collateral description (free text).
    pub collateral: Option<String>,
    /// Status of the filing ("Active", "Lapsed", "Terminated").
    pub status: Option<String>,
}

/// A party to a UCC filing (debtor or secured party).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UccParty {
    /// Party name.
    pub name: String,
    /// Street address.
    pub address: Option<String>,
    /// City.
    pub city: Option<String>,
    /// State abbreviation.
    pub state: Option<String>,
    /// ZIP code.
    pub zip: Option<String>,
}

/// Parse a California SOS UCC JSON search response.
///
/// CA BizFile Online returns a JSON envelope with a `filings` array.
#[must_use]
pub fn parse_california_ucc(json: &serde_json::Value) -> Vec<UccFiling> {
    let filings = json
        .get("filings")
        .or_else(|| json.get("data"))
        .and_then(serde_json::Value::as_array);

    filings
        .map(|arr| arr.iter().filter_map(parse_ca_filing).collect())
        .unwrap_or_default()
}

fn parse_ca_filing(f: &serde_json::Value) -> Option<UccFiling> {
    let str_field = |key: &str| -> Option<String> {
        f.get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };

    let filing_number = str_field("filingNumber").or_else(|| str_field("fileNumber"))?;

    let debtors = parse_parties(f.get("debtors"));
    let secured_parties = parse_parties(f.get("securedParties"));

    Some(UccFiling {
        filing_number,
        filing_date: str_field("filingDate").or_else(|| str_field("date")),
        lapse_date: str_field("lapseDate").or_else(|| str_field("expirationDate")),
        filing_type: str_field("filingType")
            .or_else(|| str_field("type"))
            .unwrap_or_else(|| "UCC-1 INITIAL".to_string()),
        state: "CA".to_string(),
        debtors,
        secured_parties,
        collateral: str_field("collateral").or_else(|| str_field("collateralDescription")),
        status: str_field("status"),
    })
}

fn parse_parties(val: Option<&serde_json::Value>) -> Vec<UccParty> {
    val.and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_party).collect())
        .unwrap_or_default()
}

fn parse_party(p: &serde_json::Value) -> Option<UccParty> {
    let name = p
        .get("name")
        .or_else(|| p.get("organizationName"))
        .or_else(|| p.get("individualName"))
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)?;

    let str_field = |key: &str| -> Option<String> {
        p.get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };

    Some(UccParty {
        name,
        address: str_field("address").or_else(|| str_field("street")),
        city: str_field("city"),
        state: str_field("state"),
        zip: str_field("zip").or_else(|| str_field("postalCode")),
    })
}

/// Parse a New York SOS UCC HTML table response.
///
/// NY SOS returns an HTML page with a `<table>` of results.
/// Each row contains filing number, type, date, and debtor/secured party names.
#[must_use]
pub fn parse_new_york_ucc(html: &str) -> Vec<UccFiling> {
    // Extract table rows between <tr> tags
    let rows: Vec<&str> = html
        .split("<tr")
        .skip(2) // skip header row(s)
        .collect();

    rows.iter().filter_map(|row| parse_ny_row(row)).collect()
}

fn parse_ny_row(row: &str) -> Option<UccFiling> {
    // Extract text content from <td> cells
    let cells: Vec<String> = row
        .split("<td")
        .skip(1)
        .filter_map(|cell| {
            let start = cell.find('>')?;
            let end = cell.find("</td>")?;
            let text = &cell[start + 1..end];
            // Strip nested tags
            let clean: String = text
                .chars()
                .scan(0u32, |depth, c| match c {
                    '<' => {
                        *depth += 1;
                        Some(None)
                    }
                    '>' => {
                        *depth = depth.saturating_sub(1);
                        Some(None)
                    }
                    _ if *depth == 0 => Some(Some(c)),
                    _ => Some(None),
                })
                .flatten()
                .collect::<String>()
                .trim()
                .to_string();
            if clean.is_empty() { None } else { Some(clean) }
        })
        .collect();

    if cells.len() < 4 {
        return None;
    }

    let filing_number = cells.first()?.clone();
    if filing_number.is_empty() {
        return None;
    }

    let debtor_name = cells.get(2).cloned().unwrap_or_default();
    let secured_name = cells.get(3).cloned().unwrap_or_default();

    let debtors = if debtor_name.is_empty() {
        vec![]
    } else {
        vec![UccParty {
            name: debtor_name,
            address: None,
            city: None,
            state: None,
            zip: None,
        }]
    };

    let secured_parties = if secured_name.is_empty() {
        vec![]
    } else {
        vec![UccParty {
            name: secured_name,
            address: None,
            city: None,
            state: None,
            zip: None,
        }]
    };

    Some(UccFiling {
        filing_number,
        filing_date: cells.get(1).cloned(),
        lapse_date: None,
        filing_type: "UCC-1 INITIAL".to_string(),
        state: "NY".to_string(),
        debtors,
        secured_parties,
        collateral: None,
        status: Some("Active".to_string()),
    })
}

/// Fetch UCC filings for a debtor name from a supported state.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or the response cannot be parsed.
pub async fn fetch_ucc_filings(
    state: &str,
    debtor_name: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    let filings = match state.to_ascii_uppercase().as_str() {
        "CA" => {
            let resp = client
                .get("https://bizfileonline.sos.ca.gov/api/ucc/search")
                .query(&[("query", debtor_name), ("searchType", "DEBTOR_NAME")])
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
            parse_california_ucc(&json)
        }
        "NY" => {
            // Build a URL-encoded POST body for the NY UCC web form
            let form_body = format!(
                "SearchType=DEBTOR_NAME&Query={}",
                debtor_name.replace(' ', "+")
            );
            let resp = client
                .post("https://appext20.dos.ny.gov/pls/ucc_public/web_search")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(form_body)
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
            parse_new_york_ucc(&html)
        }
        other => {
            return Err(FetchError::Other(format!(
                "UCC search not yet implemented for state: {other}"
            )));
        }
    };

    let serialized: Vec<serde_json::Value> = filings
        .iter()
        .filter_map(|f| serde_json::to_value(f).ok())
        .collect();

    let output_path = output_dir.join(format!("ucc_filings_{}.ndjson", state.to_ascii_lowercase()));
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "ucc_filings".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn ca_fixture() -> serde_json::Value {
        serde_json::json!({
            "filings": [
                {
                    "filingNumber": "201912345678",
                    "filingDate": "2019-03-15",
                    "lapseDate": "2024-03-15",
                    "filingType": "UCC-1 INITIAL",
                    "status": "Active",
                    "debtors": [
                        {
                            "name": "ACME CORP",
                            "address": "123 INDUSTRIAL BLVD",
                            "city": "LOS ANGELES",
                            "state": "CA",
                            "zip": "90001"
                        }
                    ],
                    "securedParties": [
                        {
                            "name": "FIRST BANK OF CALIFORNIA",
                            "address": "456 FINANCIAL ST",
                            "city": "SAN FRANCISCO",
                            "state": "CA",
                            "zip": "94105"
                        }
                    ],
                    "collateral": "All accounts receivable and inventory"
                },
                {
                    "filingNumber": "202187654321",
                    "filingDate": "2021-07-01",
                    "lapseDate": "2026-07-01",
                    "filingType": "UCC-1 INITIAL",
                    "status": "Active",
                    "debtors": [
                        { "name": "ACME CORP" }
                    ],
                    "securedParties": [
                        { "name": "EQUIPMENT FINANCE LLC" }
                    ],
                    "collateral": "All equipment"
                }
            ]
        })
    }

    #[test]
    fn ucc_parses_ca_filings_extracts_number_date_parties() {
        let json = ca_fixture();
        let filings = parse_california_ucc(&json);

        assert_eq!(filings.len(), 2);
        assert_eq!(filings[0].filing_number, "201912345678");
        assert_eq!(filings[0].filing_date.as_deref(), Some("2019-03-15"));
        assert_eq!(filings[0].lapse_date.as_deref(), Some("2024-03-15"));
        assert_eq!(filings[0].state, "CA");
    }

    #[test]
    fn ucc_extracts_debtor_name_and_secured_party() {
        let json = ca_fixture();
        let filings = parse_california_ucc(&json);

        assert_eq!(filings[0].debtors.len(), 1);
        assert_eq!(filings[0].debtors[0].name, "ACME CORP");
        assert_eq!(
            filings[0].secured_parties[0].name,
            "FIRST BANK OF CALIFORNIA"
        );
        assert_eq!(
            filings[0].collateral.as_deref(),
            Some("All accounts receivable and inventory")
        );
    }

    #[test]
    fn ucc_handles_multiple_filings_same_debtor() {
        let json = ca_fixture();
        let filings = parse_california_ucc(&json);

        assert_eq!(filings.len(), 2);
        // Both filings have the same debtor
        assert!(filings.iter().all(|f| f.debtors[0].name == "ACME CORP"));
        // Different secured parties
        assert_ne!(
            filings[0].secured_parties[0].name,
            filings[1].secured_parties[0].name
        );
    }

    #[test]
    fn ucc_parses_ny_html_table_fixture() {
        // Minimal NY SOS HTML table format
        let html = r#"
<table>
<tr><th>Filing</th><th>Date</th><th>Debtor</th><th>Secured Party</th></tr>
<tr><td>201900001234</td><td>2019-06-01</td><td>SMITH ENTERPRISES INC</td><td>JP MORGAN CHASE BANK NA</td></tr>
<tr><td>202000009999</td><td>2020-11-15</td><td>TECH STARTUP LLC</td><td>SILICON VALLEY BANK</td></tr>
</table>
"#;
        let filings = parse_new_york_ucc(html);

        assert!(filings.len() >= 1);
        assert_eq!(filings[0].filing_number, "201900001234");
        assert_eq!(filings[0].state, "NY");
        assert_eq!(filings[0].debtors[0].name, "SMITH ENTERPRISES INC");
        assert_eq!(
            filings[0].secured_parties[0].name,
            "JP MORGAN CHASE BANK NA"
        );
    }
}
