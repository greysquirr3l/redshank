//! Common Crawl index query — historical web archive intelligence.
//!
//! Source: <https://index.commoncrawl.org/>
//! Public API, no authentication required. Archives ~3B pages/month.
//! For WARC content retrieval: s3://commoncrawl/ (requestor-pays).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

/// The main Common Crawl index API endpoint.
const CC_INDEX_TEMPLATE: &str = "https://index.commoncrawl.org/{crawl_id}-index";
/// The canonical "latest" crawl ID alias — resolve via the Availability API.
const CC_LATEST: &str = "CC-MAIN-2025-18";

/// A Common Crawl index record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CcRecord {
    /// Full URL of the captured page.
    pub url: String,
    /// Crawl timestamp (14-char: YYYYMMDDhhmmss).
    pub timestamp: String,
    /// MIME type.
    pub mime: Option<String>,
    /// HTTP status code.
    pub status: Option<u16>,
    /// S3 WARC filename for content retrieval.
    pub filename: Option<String>,
    /// Byte offset within the WARC file.
    pub offset: Option<u64>,
    /// Byte length of the WARC record.
    pub length: Option<u64>,
    /// Content digest (SHA-1).
    pub digest: Option<String>,
    /// Crawl ID this record belongs to.
    pub crawl: String,
}

/// Parse newline-delimited JSON records from the Common Crawl index response.
///
/// The CC index returns one JSON object per line (not an array).
#[must_use]
pub fn parse_cc_index_response(text: &str, crawl_id: &str) -> Vec<CcRecord> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let json: serde_json::Value = serde_json::from_str(line).ok()?;
            parse_cc_record(&json, crawl_id)
        })
        .collect()
}

fn parse_cc_record(json: &serde_json::Value, crawl_id: &str) -> Option<CcRecord> {
    let url = json.get("url").and_then(serde_json::Value::as_str)?.to_string();
    let timestamp = json
        .get("timestamp")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let status = json
        .get("status")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| v.as_u64().map(|n| n as u16))
        });

    let offset = json
        .get("offset")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| v.as_u64())
        });

    let length = json
        .get("length")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| v.as_u64())
        });

    Some(CcRecord {
        url,
        timestamp,
        mime: json
            .get("mime")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        status,
        filename: json
            .get("filename")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        offset,
        length,
        digest: json
            .get("digest")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        crawl: crawl_id.to_string(),
    })
}

/// Query the Common Crawl index for archived URLs matching a domain pattern.
///
/// # Arguments
///
/// * `url_pattern` — URL to search (e.g. `"example.com/*"` or `"*.example.com"`).
/// * `crawl_id` — Crawl identifier (e.g. `"CC-MAIN-2025-18"`); defaults to `CC-MAIN-2025-18`.
/// * `output_dir` — Directory for NDJSON output.
/// * `rate_limit_ms` — Minimum delay between requests.
///
/// # Errors
///
/// Returns `Err` if the request fails or returns a non-2xx status.
pub async fn fetch_cc_index(
    url_pattern: &str,
    crawl_id: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let crawl = crawl_id.unwrap_or(CC_LATEST);
    let endpoint = CC_INDEX_TEMPLATE.replace("{crawl_id}", crawl);

    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    let resp = client
        .get(&endpoint)
        .query(&[
            ("url", url_pattern),
            ("output", "json"),
            ("limit", "1000"),
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

    let text = resp.text().await?;
    let records = parse_cc_index_response(&text, crawl);

    let serialized: Vec<serde_json::Value> = records
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let slug = url_pattern
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(40)
        .collect::<String>();
    let output_path = output_dir.join(format!("commoncrawl_{slug}.ndjson"));
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "common_crawl".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn ndnl_fixture() -> &'static str {
        r#"{"url":"https://example.com/page1","timestamp":"20250318123456","mime":"text/html","status":"200","filename":"crawl-data/CC-MAIN-2025-18/segments/111/warc/CC-001.warc.gz","offset":"12345678","length":"4321","digest":"sha1:AABBCC1122334455"}
{"url":"https://example.com/old-page","timestamp":"20250101000001","mime":"text/html","status":"301","filename":"crawl-data/CC-MAIN-2025-18/segments/222/warc/CC-002.warc.gz","offset":"987654","length":"1234"}
{"url":"https://example.com/missing","timestamp":"20250102120000","mime":"text/html","status":"404","filename":"crawl-data/CC-MAIN-2025-18/segments/333/warc/CC-003.warc.gz","offset":"111","length":"222"}"#
    }

    #[test]
    fn commoncrawl_constructs_correct_index_query_and_parses_response() {
        let records = parse_cc_index_response(ndnl_fixture(), "CC-MAIN-2025-18");

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].url, "https://example.com/page1");
        assert_eq!(records[0].timestamp, "20250318123456");
        assert_eq!(records[0].status, Some(200));
        assert!(records[0].filename.as_deref().unwrap().contains("CC-001"));
        assert_eq!(records[0].crawl, "CC-MAIN-2025-18");
    }

    #[test]
    fn commoncrawl_parses_offset_and_length_for_warc_retrieval() {
        let records = parse_cc_index_response(ndnl_fixture(), "CC-MAIN-2025-18");

        assert_eq!(records[0].offset, Some(12_345_678));
        assert_eq!(records[0].length, Some(4321));
        assert_eq!(
            records[0].digest.as_deref(),
            Some("sha1:AABBCC1122334455")
        );
    }

    #[test]
    fn commoncrawl_parses_redirect_and_not_found_status_codes() {
        let records = parse_cc_index_response(ndnl_fixture(), "CC-MAIN-2025-18");

        assert_eq!(records[1].status, Some(301));
        assert_eq!(records[2].status, Some(404));
    }
}
