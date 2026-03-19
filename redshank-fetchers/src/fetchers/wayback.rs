//! Wayback Machine CDX Server — Historical web archive snapshots.
//!
//! API: `https://web.archive.org/cdx/search/cdx`
//! Params: `url={domain_or_url}&output=json&limit=500`
//! No auth, no stated rate limit — be polite (500ms between requests).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const CDX_API: &str = "https://web.archive.org/cdx/search/cdx";
/// Polite delay between Wayback requests.
const WAYBACK_DELAY_MS: u64 = 500;

/// Fetch archived snapshots of a URL from the Wayback Machine.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_wayback_snapshots(
    url: &str,
    output_dir: &Path,
    max_records: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let limit = if max_records == 0 { 500 } else { max_records };

    // Enforce polite delay
    tokio::time::sleep(std::time::Duration::from_millis(WAYBACK_DELAY_MS)).await;

    let resp = client
        .get(CDX_API)
        .query(&[
            ("url", url),
            ("output", "json"),
            ("limit", &limit.to_string()),
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
    let records = parse_cdx_response(&json);

    let output_path = output_dir.join("wayback_snapshots.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "wayback".into(),
    })
}

/// CDX API returns an array of arrays. First row is headers, rest are data.
/// Headers: \[urlkey, timestamp, original, mimetype, statuscode, digest, length\]
#[must_use]
pub fn parse_cdx_response(json: &serde_json::Value) -> Vec<serde_json::Value> {
    let rows = match json.as_array() {
        Some(r) if r.len() > 1 => r,
        _ => return Vec::new(),
    };

    let headers: Vec<&str> = rows
        .first()
        .and_then(|r| r.as_array())
        .map(|h| h.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    rows.get(1..)
        .unwrap_or_default()
        .iter()
        .filter_map(|row| {
            let values = row.as_array()?;
            let mut record = serde_json::Map::new();
            for (i, header) in headers.iter().enumerate() {
                if let Some(val) = values.get(i) {
                    record.insert((*header).to_owned(), val.clone());
                }
            }
            Some(serde_json::Value::Object(record))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn wayback_cdx_parses_array_of_arrays_into_records() {
        let mock = serde_json::json!([
            [
                "urlkey",
                "timestamp",
                "original",
                "mimetype",
                "statuscode",
                "digest",
                "length"
            ],
            [
                "com,example)/",
                "20200101120000",
                "https://example.com/",
                "text/html",
                "200",
                "ABC123",
                "5432"
            ],
            [
                "com,example)/about",
                "20200615080000",
                "https://example.com/about",
                "text/html",
                "200",
                "DEF456",
                "3210"
            ],
        ]);
        let records = parse_cdx_response(&mock);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["original"], "https://example.com/");
        assert_eq!(records[0]["timestamp"], "20200101120000");
        assert_eq!(records[0]["statuscode"], "200");
        assert_eq!(records[1]["urlkey"], "com,example)/about");
    }

    #[test]
    fn wayback_cdx_handles_empty_response() {
        let mock = serde_json::json!([]);
        let records = parse_cdx_response(&mock);
        assert!(records.is_empty());
    }
}
