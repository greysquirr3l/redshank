//! FEC — Federal Election Commission campaign finance data.
//!
//! API: <https://api.open.fec.gov/v1/>
//! Pagination: page-based (1-indexed), per_page max 100.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.open.fec.gov/v1";
const DEFAULT_PER_PAGE: u32 = 100;

/// Fetch FEC candidate data for the given query.
pub async fn fetch_candidates(
    query: &str,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let resp = client
            .get(format!("{API_BASE}/candidates/search/"))
            .query(&[
                ("q", query),
                ("api_key", api_key),
                ("page", &page.to_string()),
                ("per_page", &DEFAULT_PER_PAGE.to_string()),
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
        let results = json
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if results.is_empty() {
            break;
        }
        all_records.extend(results);

        let total_pages = json
            .get("pagination")
            .and_then(|p| p.get("pages"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        if page >= total_pages {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("fec_candidates.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "fec".into(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn fec_parses_candidate_search_response() {
        let mock_json = serde_json::json!({
            "results": [
                {"candidate_id": "H0OH01234", "name": "DOE, JOHN", "party": "REP", "state": "OH"},
                {"candidate_id": "S0OH05678", "name": "SMITH, JANE", "party": "DEM", "state": "OH"},
            ],
            "pagination": {"pages": 1, "page": 1, "count": 2, "per_page": 100}
        });
        let results = mock_json
            .get("results")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["name"], "DOE, JOHN");
        assert_eq!(results[1]["party"], "DEM");
    }
}
