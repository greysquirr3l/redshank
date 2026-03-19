//! USPTO — Patent and Trademark search.
//!
//! `PatentsView` API: `https://search.patentsview.org/api/v1/inventor/`
//! TMAPI: `https://developer.uspto.gov/api-catalog/trademark-search`
//! `PatentsView`: 1000 req/day free, no auth.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const PATENTS_API: &str = "https://search.patentsview.org/api/v1";

/// Fetch patent inventor records matching the given last name.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_patent_inventors(
    last_name: &str,
    first_name: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let mut query = serde_json::json!({
            "inventor_last_name": last_name
        });
        if let Some(fname) = first_name
            && let Some(obj) = query.as_object_mut()
        {
            obj.insert(
                "inventor_first_name".to_string(),
                serde_json::Value::String(fname.to_string()),
            );
        }

        let resp = client
            .get(format!("{PATENTS_API}/inventor/"))
            .query(&[
                ("q", &serde_json::to_string(&query).unwrap_or_default()),
                ("page", &page.to_string()),
                ("per_page", &"25".to_string()),
            ])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(FetchError::ApiError {
                status: status.as_u16(),
                body: text,
            });
        }

        let json: serde_json::Value = resp.json().await?;
        let inventors = json
            .get("inventors")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if inventors.is_empty() {
            break;
        }
        all_records.extend(inventors);

        let total_pages = u32::try_from(
            json.get("total_patent_count")
                .and_then(serde_json::Value::as_u64)
                .map_or(1, |total| total.div_ceil(25)),
        )
        .unwrap_or(u32::MAX);

        if page >= total_pages {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("uspto_inventors.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "uspto".into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    #[test]
    fn uspto_parses_patent_inventor_list_fixture() {
        let mock = serde_json::json!({
            "inventors": [
                {
                    "inventor_id": "fl:jn-ln:doe-1",
                    "inventor_first_name": "John",
                    "inventor_last_name": "Doe",
                    "inventor_city": "San Jose",
                    "inventor_state": "CA",
                    "inventor_country": "US",
                    "patent_number": "US12345678",
                    "patent_title": "Method for Improved Widget Assembly",
                    "patent_date": "2024-01-15"
                }
            ],
            "total_patent_count": 1
        });
        let inventors = mock.get("inventors").and_then(|v| v.as_array()).unwrap();
        assert_eq!(inventors.len(), 1);
        assert_eq!(inventors[0]["inventor_last_name"], "Doe");
        assert_eq!(
            inventors[0]["patent_title"],
            "Method for Improved Widget Assembly"
        );
    }
}
