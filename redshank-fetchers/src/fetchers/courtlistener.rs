//! CourtListener / RECAP — Free federal court archive.
//!
//! API: `https://www.courtlistener.com/api/rest/v4/dockets/`
//! No auth for basic use; API key for bulk (5000 req/day).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://www.courtlistener.com/api/rest/v4";

/// Fetch federal court dockets matching the given query string.
pub async fn fetch_dockets(
    query: &str,
    api_token: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };
    let mut next_url: Option<String> = None;

    for page in 1..=max {
        let url = next_url.unwrap_or_else(|| format!("{API_BASE}/dockets/"));
        let mut req = client.get(&url);

        if page == 1 {
            req = req.query(&[("q", query)]);
        }
        if let Some(token) = api_token {
            req = req.header("Authorization", format!("Token {token}"));
        }

        let resp = req.send().await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(FetchError::ApiError {
                status: status.as_u16(),
                body: text,
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

        // CourtListener uses cursor-based pagination with a `next` URL
        next_url = json
            .get("next")
            .and_then(|v| v.as_str())
            .map(String::from);

        if next_url.is_none() {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("courtlistener_dockets.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "courtlistener".into(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn courtlistener_parses_docket_response() {
        let mock = serde_json::json!({
            "count": 42,
            "next": null,
            "results": [
                {
                    "id": 12345,
                    "case_name": "United States v. ACME Corp",
                    "court": "txsd",
                    "date_filed": "2023-06-15",
                    "docket_number": "4:23-cv-01234",
                    "parties": [
                        {"name": "ACME CORP", "type": "Defendant"},
                        {"name": "United States", "type": "Plaintiff"}
                    ]
                }
            ]
        });
        let results = mock.get("results").and_then(|v| v.as_array()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["case_name"], "United States v. ACME Corp");
        assert_eq!(results[0]["court"], "txsd");
        let parties = results[0]["parties"].as_array().unwrap();
        assert_eq!(parties[0]["name"], "ACME CORP");
    }
}
