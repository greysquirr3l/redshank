//! SEC Schedule 13D and 13G parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SUBMISSIONS_BASE: &str = "https://data.sec.gov/submissions";

/// A normalized beneficial ownership filing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct BeneficialOwnershipFiling {
    pub form_type: String,
    pub filing_date: Option<String>,
    pub issuer: Option<String>,
    pub filer: Option<String>,
    pub ownership_percentage: Option<f64>,
    pub shares_owned: Option<u64>,
    pub purpose_statement: Option<String>,
    pub amendment: bool,
    pub passive: bool,
}

/// Filter SEC submissions JSON for Schedule 13D/13G filings.
#[must_use]
pub fn extract_ownership_filings(json: &serde_json::Value) -> Vec<BeneficialOwnershipFiling> {
    let recent = json
        .get("filings")
        .and_then(|filings| filings.get("recent"))
        .unwrap_or(&serde_json::Value::Null);

    let forms = recent
        .get("form")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let dates = recent
        .get("filingDate")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    forms
        .iter()
        .enumerate()
        .filter_map(|(index, form)| {
            let form_type = form.as_str()?;
            if !matches!(form_type, "SC 13D" | "SC 13D/A" | "SC 13G" | "SC 13G/A") {
                return None;
            }

            Some(BeneficialOwnershipFiling {
                form_type: form_type.to_string(),
                filing_date: dates
                    .get(index)
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                issuer: json
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                filer: None,
                ownership_percentage: None,
                shares_owned: None,
                purpose_statement: None,
                amendment: form_type.ends_with("/A"),
                passive: form_type.contains("13G"),
            })
        })
        .collect()
}

fn extract_percent(text: &str) -> Option<f64> {
    let marker = "percent of class represented by amount in row";
    let lower = text.to_ascii_lowercase();
    let marker_pos = lower.find(marker)?;
    let window = text
        .get(marker_pos..)?
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    window
        .split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .find(|token| !token.is_empty() && token.contains('.'))
        .or_else(|| {
            window
                .split_whitespace()
                .find(|token| token.chars().all(|ch| ch.is_ascii_digit()))
        })
        .and_then(|token| token.parse::<f64>().ok())
}

fn extract_shares(text: &str) -> Option<u64> {
    let marker = "aggregate amount beneficially owned";
    let lower = text.to_ascii_lowercase();
    let marker_pos = lower.find(marker)?;
    let window = text
        .get(marker_pos..)?
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    window
        .split(|ch: char| !(ch.is_ascii_digit() || ch == ','))
        .find(|token| token.chars().any(|ch| ch.is_ascii_digit()))
        .and_then(|token| token.replace(',', "").parse::<u64>().ok())
}

/// Parse filing body text for filer, issuer, ownership percentage, purpose, and amendment state.
#[must_use]
pub fn parse_filing_text(form_type: &str, filing_text: &str) -> BeneficialOwnershipFiling {
    let lines: Vec<&str> = filing_text.lines().map(str::trim).collect();

    let labeled_value = |label: &str| {
        lines
            .iter()
            .position(|line| line.eq_ignore_ascii_case(label))
            .and_then(|index| lines.get(index + 1))
            .copied()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    let purpose_statement = filing_text
        .split("Item 4.")
        .nth(1)
        .and_then(|tail| tail.split("Item 5.").next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    BeneficialOwnershipFiling {
        form_type: form_type.to_string(),
        filing_date: None,
        issuer: labeled_value("Name of Issuer"),
        filer: labeled_value("Name of Reporting Person"),
        ownership_percentage: extract_percent(filing_text),
        shares_owned: extract_shares(filing_text),
        purpose_statement,
        amendment: form_type.ends_with("/A"),
        passive: form_type.contains("13G"),
    }
}

/// Fetch SEC submissions and persist filtered 13D/13G filing metadata.
///
/// # Errors
///
/// Returns `Err` if the request fails or the response status is non-success.
pub async fn fetch_filings(cik: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let url = format!("{SUBMISSIONS_BASE}/{cik}.json");
    let resp = client.get(url).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let records = extract_ownership_filings(&json)
        .into_iter()
        .map(|record| {
            serde_json::to_value(record).map_err(|err| FetchError::Parse(err.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let output_path = output_dir.join("sec_13d_13g.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "sec-13d-13g".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn submissions_fixture() -> serde_json::Value {
        serde_json::json!({
            "name": "Example Issuer Inc.",
            "filings": {
                "recent": {
                    "form": ["10-K", "SC 13D", "SC 13G/A"],
                    "filingDate": ["2024-02-01", "2024-03-10", "2024-06-01"]
                }
            }
        })
    }

    fn filing_text_fixture() -> &'static str {
        r"
SCHEDULE 13D
Name of Issuer
Example Issuer Inc.
Name of Reporting Person
Northwind Capital LP
Aggregate Amount Beneficially Owned by Each Reporting Person
1,250,000
Percent of Class Represented by Amount in Row
7.4
Item 4.
The reporting person intends to seek board representation and evaluate strategic alternatives, including a possible merger or asset sale.
Item 5.
Interest in Securities of the Issuer.
    "
    }

    #[test]
    fn sec_13d_fetcher_parses_filing_fixture_extracts_filer_and_ownership_percentage() {
        let filing = parse_filing_text("SC 13D", filing_text_fixture());

        assert_eq!(filing.filer.as_deref(), Some("Northwind Capital LP"));
        assert_eq!(filing.issuer.as_deref(), Some("Example Issuer Inc."));
        assert_eq!(filing.ownership_percentage, Some(7.4));
        assert_eq!(filing.shares_owned, Some(1_250_000));
    }

    #[test]
    fn sec_13d_fetcher_distinguishes_13d_from_13g() {
        let filings = extract_ownership_filings(&submissions_fixture());

        assert_eq!(filings.len(), 2);
        assert!(!filings[0].passive);
        assert!(filings[1].passive);
        assert!(filings[1].amendment);
    }

    #[test]
    fn sec_13d_fetcher_extracts_purpose_statement_and_amendments() {
        let filing = parse_filing_text("SC 13D/A", filing_text_fixture());

        assert!(filing.amendment);
        assert!(
            filing
                .purpose_statement
                .as_deref()
                .unwrap()
                .contains("board representation")
        );
        assert!(
            filing
                .purpose_statement
                .as_deref()
                .unwrap()
                .contains("possible merger")
        );
    }
}
