//! `ProPublica` 990 — Nonprofit tax return data.
//!
//! API: <https://projects.propublica.org/nonprofits/api/v2/>
//! Pagination: page-based (0-indexed), 25 results per page.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://projects.propublica.org/nonprofits/api/v2";

/// Fetch `ProPublica` nonprofit 990 data.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_nonprofits(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let resp = client
            .get(format!("{API_BASE}/search.json"))
            .query(&[("q", query), ("page", &page.to_string())])
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
        let orgs = json
            .get("organizations")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if orgs.is_empty() {
            break;
        }
        all_records.extend(orgs);

        let total = usize::try_from(
            json.get("total_results")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        )
        .unwrap_or(usize::MAX);

        if all_records.len() >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("propublica_990.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "propublica-990".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    #[test]
    fn propublica_parses_search_response() {
        let mock = serde_json::json!({
            "total_results": 2,
            "organizations": [
                {"ein": "123456789", "name": "ACME FOUNDATION", "state": "NY", "ntee_code": "T70"},
                {"ein": "987654321", "name": "SMITH FAMILY TRUST", "state": "CA", "ntee_code": "T20"},
            ]
        });
        let orgs = mock["organizations"].as_array().unwrap();
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0]["name"], "ACME FOUNDATION");
    }
}
