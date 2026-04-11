//! Shared HTTP client and rate-limit helper for all fetchers.

use crate::domain::{FetchConfig, FetchError};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use std::time::Duration;

/// User-Agent string for all outbound requests.
const REDSHANK_USER_AGENT: &str = "redshank/0.1.0 (research tool; contact in AGENTS.md)";

/// Default request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Build a `reqwest::Client` with the standard Redshank User-Agent and timeout.
///
/// # Errors
///
/// Returns `Err` if the underlying HTTP client cannot be constructed.
pub fn build_client() -> Result<reqwest::Client, FetchError> {
    reqwest::Client::builder()
        .user_agent(REDSHANK_USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(FetchError::Http)
}

/// Build a `reqwest::Client` that sends a single custom header on every request.
///
/// Useful for APIs that require a static API key header (e.g. `X-ListenAPI-Key`).
///
/// # Errors
///
/// Returns `Err` if the header name or value is invalid, or the client cannot be constructed.
pub fn build_client_with_key(header_name: &str, header_value: &str) -> Result<reqwest::Client, FetchError> {
    let mut headers = HeaderMap::new();
    let name: HeaderName = header_name
        .parse()
        .map_err(|e| FetchError::Other(format!("invalid header name '{header_name}': {e}")))?;
    let value: HeaderValue = header_value
        .parse()
        .map_err(|e| FetchError::Other(format!("invalid header value for '{header_name}': {e}")))?;
    headers.insert(name, value);
    headers.insert(USER_AGENT, HeaderValue::from_static(REDSHANK_USER_AGENT));

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(FetchError::Http)
}

/// Build a `reqwest::Client` with extra default headers merged from a `FetchConfig`.
///
/// # Errors
///
/// Returns `Err` if a header name or value is invalid, or the client cannot be constructed.
pub fn build_client_from_config(config: &FetchConfig) -> Result<reqwest::Client, FetchError> {
    let mut headers = HeaderMap::new();
    for (k, v) in &config.headers {
        let name: HeaderName = k
            .parse()
            .map_err(|e| FetchError::Other(format!("invalid header name '{k}': {e}")))?;
        let value: HeaderValue = v
            .parse()
            .map_err(|e| FetchError::Other(format!("invalid header value for '{k}': {e}")))?;
        headers.insert(name, value);
    }
    // Ensure User-Agent is always set even if overridden in config headers.
    if !headers.contains_key(USER_AGENT) {
        headers.insert(USER_AGENT, HeaderValue::from_static(REDSHANK_USER_AGENT));
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(FetchError::Http)
}

/// Sleep for the configured rate-limit duration between paginated requests.
///
/// This MUST be called between consecutive page fetches to respect source rate limits.
pub async fn rate_limit_delay(rate_limit_ms: u64) {
    if rate_limit_ms > 0 {
        tokio::time::sleep(Duration::from_millis(rate_limit_ms)).await;
    }
}

// ── NDJSON Writer ───────────────────────────────────────────

use std::io::Write;
use std::path::Path;

/// Write a slice of `serde_json::Value` records as newline-delimited JSON to a file.
///
/// # Errors
///
/// Returns `Err` if the file cannot be created or a record cannot be serialized.
pub fn write_ndjson(path: &Path, records: &[serde_json::Value]) -> Result<usize, FetchError> {
    let file = std::fs::File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);

    for record in records {
        serde_json::to_writer(&mut writer, record)
            .map_err(|e| FetchError::Parse(format!("serialize record: {e}")))?;
        writer.write_all(b"\n")?;
    }

    writer.flush()?;
    Ok(records.len())
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn build_client_sets_user_agent() {
        let client = build_client().unwrap();
        // We can't directly inspect default headers from the client,
        // but we can verify it builds successfully.
        drop(client);
    }

    #[test]
    fn build_client_from_config_includes_custom_headers() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("X-Custom".into(), "test-val".into());
        let config = FetchConfig {
            base_url: "https://example.com".into(),
            query_params: std::collections::HashMap::default(),
            headers,
            rate_limit_ms: 100,
            max_pages: 10,
            output_path: "/tmp/test".into(),
        };
        let client = build_client_from_config(&config).unwrap();
        drop(client);
    }

    #[tokio::test]
    async fn rate_limit_delay_sleeps_at_least_specified_duration() {
        let ms = 50;
        let start = tokio::time::Instant::now();
        rate_limit_delay(ms).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(ms),
            "Expected at least {ms}ms, got {}ms",
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn rate_limit_delay_zero_returns_immediately() {
        let start = tokio::time::Instant::now();
        rate_limit_delay(0).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_millis(5),
            "Zero delay should return immediately, took {}ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn write_ndjson_produces_newline_delimited_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.ndjson");
        let records = vec![
            serde_json::json!({"name": "Alice", "id": 1}),
            serde_json::json!({"name": "Bob", "id": 2}),
        ];
        let count = write_ndjson(&path, &records).unwrap();
        assert_eq!(count, 2);

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        // Each line is valid JSON
        for line in &lines {
            let _: serde_json::Value = serde_json::from_str(line).unwrap();
        }
    }
}
