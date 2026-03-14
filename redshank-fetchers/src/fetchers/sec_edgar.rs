//! SEC EDGAR — Securities and Exchange Commission filing data.
//!
//! API: <https://data.sec.gov/submissions/{CIK}.json>
//! Ticker→CIK: <https://www.sec.gov/files/company_tickers.json>
//! No pagination (full entity fetch per CIK).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const SUBMISSIONS_BASE: &str = "https://data.sec.gov/submissions";

/// Resolve a ticker symbol to a 10-digit CIK string.
pub fn resolve_ticker(tickers_json: &serde_json::Value, ticker: &str) -> Option<String> {
    let upper = ticker.to_uppercase();
    if let Some(obj) = tickers_json.as_object() {
        for entry in obj.values() {
            if entry.get("ticker").and_then(|t| t.as_str()) == Some(&upper)
                && let Some(cik) = entry.get("cik_str").and_then(|c| c.as_u64())
            {
                return Some(format!("CIK{cik:010}"));
            }
        }
    }
    None
}

/// Fetch SEC EDGAR submissions for a CIK.
pub async fn fetch_submissions(
    cik: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let url = format!("{SUBMISSIONS_BASE}/{cik}.json");

    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let records = vec![json];

    let output_path = output_dir.join("sec_edgar.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "sec-edgar".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sec_edgar_ticker_resolution_parses_company_tickers_json() {
        let fixture = serde_json::json!({
            "0": {"cik_str": 320193, "ticker": "AAPL", "title": "Apple Inc."},
            "1": {"cik_str": 789019, "ticker": "MSFT", "title": "MICROSOFT CORP"},
        });
        let cik = resolve_ticker(&fixture, "AAPL").unwrap();
        assert_eq!(cik, "CIK0000320193");

        let cik2 = resolve_ticker(&fixture, "msft").unwrap();
        assert_eq!(cik2, "CIK0000789019");

        assert!(resolve_ticker(&fixture, "ZZZZZ").is_none());
    }
}
