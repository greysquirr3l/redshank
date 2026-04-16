//! Browser-based search fallback for API failures.
//!
//! When a structured data API returns an error or becomes unavailable,
//! `BrowserSearchFallback` drives the website directly using a headless
//! Chrome pool via `stygian-browser`. Results are returned as raw page
//! content or embedded JSON blocks for the caller to parse.
//!
//! # Usage
//!
//! ```no_run
//! use redshank_core::adapters::providers::browser_fallback::{
//!     BrowserSearchConfig, BrowserSearchFallback,
//! };
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let fallback = BrowserSearchFallback::new();
//! let config = BrowserSearchConfig {
//!     search_url: "https://efts.fec.gov/public/search?q={query}".into(),
//!     extra_wait_ms: 1_500,
//!     timeout_secs: 45,
//! };
//! let results = fallback.on_api_failure("503 Service Unavailable", &config, "defense").await?;
//! println!("{} record(s) extracted", results.len());
//! # Ok(())
//! # }
//! ```
//!
//! Requires the `stygian` feature flag.

use crate::domain::errors::DomainError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use stygian_browser::{BrowserConfig, BrowserPool, WaitUntil};
use tokio::sync::OnceCell;

// ── Configuration ────────────────────────────────────────────────────────────

/// Configuration for a browser-based search fallback on a specific website.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSearchConfig {
    /// URL template for the site's search page.
    ///
    /// Use `{query}` as a placeholder for the URL-encoded search term, e.g.:
    /// `"https://efts.fec.gov/public/search?q={query}"`.
    pub search_url: String,

    /// Extra milliseconds to wait after `NetworkIdle` for any post-load JS rendering.
    ///
    /// Defaults to `1500`.
    #[serde(default = "default_extra_wait_ms")]
    pub extra_wait_ms: u64,

    /// Request timeout in seconds.
    ///
    /// Defaults to `45`.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

const fn default_extra_wait_ms() -> u64 {
    1_500
}
const fn default_timeout_secs() -> u64 {
    45
}

// ── Result ───────────────────────────────────────────────────────────────────

/// The raw result from a browser fetch or search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSearchResult {
    /// The final URL (after any redirects).
    pub url: String,
    /// Page `<title>` text.
    pub title: String,
    /// Full page source (HTML or text depending on what the browser captured).
    pub content: String,
    /// Always `true` — page was JS-rendered via the browser pool.
    pub rendered: bool,
}

impl BrowserSearchResult {
    /// Convert this result to a metadata-only `Value` (without the full content).
    ///
    /// Use this as a lightweight record when the caller just needs provenance.
    pub fn to_record(&self) -> Value {
        json!({
            "url": self.url,
            "title": self.title,
            "content_length": self.content.len(),
            "rendered": self.rendered,
            "source": "browser_fallback",
        })
    }

    /// Convert this result to a `Value` that includes the full page content.
    ///
    /// Use this when the caller (e.g. an LLM agent) will parse the content itself.
    pub fn to_content_record(&self) -> Value {
        json!({
            "url": self.url,
            "title": self.title,
            "content": self.content,
            "rendered": self.rendered,
            "source": "browser_fallback",
        })
    }

    /// Extract embedded JSON objects from `<script>` tags in the page source.
    ///
    /// Many government and corporate data sites embed their API response directly
    /// in `<script type="application/json">` or `window.__DATA__ = {...}` blocks.
    /// This method finds all top-level JSON objects inside script elements.
    ///
    /// Returns an empty `Vec` if no JSON blocks are found or none parse cleanly.
    pub fn extract_json_blocks(&self) -> Vec<Value> {
        use regex::Regex;
        // Match content of <script> tags that starts with `{` or `[`.
        let re = Regex::new(r#"<script[^>]*>\s*(\{[\s\S]*?\}|\[[\s\S]*?\])\s*</script>"#)
            .expect("browser fallback JSON regex is valid ASCII");
        re.captures_iter(&self.content)
            .filter_map(|cap| {
                cap.get(1)
                    .and_then(|m| serde_json::from_str::<Value>(m.as_str()).ok())
            })
            .collect()
    }

    /// Extract text blocks that start with `window.` assignments common in SPAs.
    ///
    /// Returns a `Vec<Value>` of parsed objects from `window.__PRELOADED_STATE__`,
    /// `window.__INITIAL_STATE__`, `window.__DATA__`, and similar patterns.
    pub fn extract_window_data(&self) -> Vec<Value> {
        use regex::Regex;
        let re = Regex::new(r#"window\.__[A-Z_]+__\s*=\s*(\{[\s\S]*?\});"#)
            .expect("browser fallback window data regex is valid");
        re.captures_iter(&self.content)
            .filter_map(|cap| {
                cap.get(1)
                    .and_then(|m| serde_json::from_str::<Value>(m.as_str()).ok())
            })
            .collect()
    }
}

// ── Fallback ─────────────────────────────────────────────────────────────────

/// Reusable browser-based fallback for API search failures.
///
/// Drives the actual website via a lazily-initialised headless Chrome pool
/// (`stygian-browser`) and returns raw page content for callers to parse.
///
/// The browser pool is shared across all method calls on a single instance
/// and is created on first use. Safe to clone — all clones share the same pool.
///
/// Requires the `stygian` feature flag.
#[derive(Clone)]
pub struct BrowserSearchFallback {
    pool: Arc<OnceCell<Arc<BrowserPool>>>,
}

impl BrowserSearchFallback {
    /// Create a new fallback handle. The browser pool is NOT started until the first use.
    pub fn new() -> Self {
        Self {
            pool: Arc::new(OnceCell::new()),
        }
    }

    async fn pool(&self) -> Result<Arc<BrowserPool>, DomainError> {
        let pool = self
            .pool
            .get_or_try_init(|| async {
                let config = BrowserConfig::default();
                BrowserPool::new(config)
                    .await
                    .map_err(|e| DomainError::Other(format!("browser pool init: {e}")))
            })
            .await?;
        Ok(Arc::clone(pool))
    }

    /// Fetch an arbitrary URL and return the rendered page content.
    ///
    /// Use this variant when you already know the exact URL to fetch (e.g. a
    /// detail page, a direct search endpoint, or a sitemap).
    pub async fn fetch_page(&self, url: &str) -> Result<BrowserSearchResult, DomainError> {
        self.fetch_page_with_timeout(url, Duration::from_secs(45), 0)
            .await
    }

    /// Fetch an arbitrary URL with explicit timeout and extra post-load wait.
    pub async fn fetch_page_with_timeout(
        &self,
        url: &str,
        timeout: Duration,
        extra_wait_ms: u64,
    ) -> Result<BrowserSearchResult, DomainError> {
        let pool = self.pool().await?;
        let handle = pool
            .acquire()
            .await
            .map_err(|e| DomainError::Other(format!("acquire browser: {e}")))?;
        let browser = handle
            .browser()
            .ok_or_else(|| DomainError::Other("browser handle invalid".into()))?;
        let mut page = browser
            .new_page()
            .await
            .map_err(|e| DomainError::Other(format!("new page: {e}")))?;

        page.navigate(url, WaitUntil::NetworkIdle, timeout)
            .await
            .map_err(|e| DomainError::Other(format!("navigate to {url}: {e}")))?;

        if extra_wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(extra_wait_ms)).await;
        }

        let title = match page.title().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("failed to extract page title from {url}: {e}");
                String::new()
            }
        };
        let content = match page.content().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("failed to extract page content from {url}: {e}");
                String::new()
            }
        };
        handle.release().await;

        Ok(BrowserSearchResult {
            url: url.to_string(),
            title,
            content,
            rendered: true,
        })
    }

    /// Drive the site's own search UI and return the rendered page.
    ///
    /// Builds the search URL by replacing `{query}` in `config.search_url` with
    /// a percent-encoded form of the query string, then navigates via the browser.
    pub async fn search(
        &self,
        config: &BrowserSearchConfig,
        query: &str,
    ) -> Result<BrowserSearchResult, DomainError> {
        let search_url = config.search_url.replace("{query}", &percent_encode(query));
        self.fetch_page_with_timeout(
            &search_url,
            Duration::from_secs(config.timeout_secs),
            config.extra_wait_ms,
        )
        .await
    }

    /// Search and immediately return all embedded JSON blocks found on the page.
    ///
    /// Convenience wrapper combining [`search`] + [`BrowserSearchResult::extract_json_blocks`]
    /// + [`BrowserSearchResult::extract_window_data`].
    pub async fn search_extract_json(
        &self,
        config: &BrowserSearchConfig,
        query: &str,
    ) -> Result<Vec<Value>, DomainError> {
        let result = self.search(config, query).await?;
        let mut records = result.extract_json_blocks();
        records.extend(result.extract_window_data());
        Ok(records)
    }

    /// Invoke the fallback after a named API failure, logging the triggering error.
    ///
    /// This is the primary entry point for fetchers to call when a structured API
    /// returns a non-success status or a network error. The method:
    ///
    /// 1. Logs a `WARN` with the triggering error and query for observability.
    /// 2. Navigates the real website using the browser pool.
    /// 3. Extracts any embedded JSON blocks from `<script>` tags.
    /// 4. Falls back to returning the full page as a single content record if
    ///    no JSON is found (so the agent can still parse the HTML).
    ///
    /// ```no_run
    /// # use redshank_core::adapters::providers::browser_fallback::{BrowserSearchFallback, BrowserSearchConfig};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let fallback = BrowserSearchFallback::new();
    /// let config = BrowserSearchConfig {
    ///     search_url: "https://efts.fec.gov/public/search?q={query}".into(),
    ///     extra_wait_ms: 1_500,
    ///     timeout_secs: 45,
    /// };
    /// let results = fallback
    ///     .on_api_failure("503 Service Unavailable", &config, "defense contractor")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn on_api_failure(
        &self,
        api_error: &str,
        config: &BrowserSearchConfig,
        query: &str,
    ) -> Result<Vec<Value>, DomainError> {
        tracing::warn!(
            api_error = %api_error,
            query = %query,
            fallback = "browser",
            "API search failed — invoking browser fallback",
        );
        let result = self.search(config, query).await?;
        let mut records = result.extract_json_blocks();
        records.extend(result.extract_window_data());
        if records.is_empty() {
            // No embedded JSON found — return the whole page so the LLM agent
            // can extract structure from the rendered text.
            Ok(vec![result.to_content_record()])
        } else {
            Ok(records)
        }
    }
}

impl Default for BrowserSearchFallback {
    fn default() -> Self {
        Self::new()
    }
}

// ── URL encoding helper ───────────────────────────────────────────────────────

/// Percent-encode a string for use as a URL query parameter value.
///
/// Encodes all characters except unreserved characters (`ALPHA / DIGIT / - . _ ~`)
/// per RFC 3986 §2.3. Spaces are encoded as `%20` (not `+`).
fn percent_encode(input: &str) -> String {
    input
        .bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![b as char]
            }
            _ => format!("%{b:02X}").chars().collect(),
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_encodes_spaces_and_specials() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("foo+bar"), "foo%2Bbar");
        assert_eq!(percent_encode("abc123-_.~"), "abc123-_.~");
        assert_eq!(percent_encode("café"), "caf%C3%A9");
    }

    #[test]
    fn browser_search_config_has_sensible_defaults() {
        let json = r#"{"search_url": "https://example.com/search?q={query}"}"#;
        let config: BrowserSearchConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.extra_wait_ms, 1_500);
        assert_eq!(config.timeout_secs, 45);
    }

    #[test]
    fn browser_fallback_new_does_not_start_pool() {
        // BrowserPool is lazy — construction must be infallible and non-blocking.
        let _f = BrowserSearchFallback::new();
    }

    #[test]
    fn browser_fallback_clone_shares_pool() {
        let a = BrowserSearchFallback::new();
        let b = a.clone();
        // Both share the same Arc<OnceCell> — pointer equality confirms sharing.
        assert!(Arc::ptr_eq(&a.pool, &b.pool));
    }

    #[test]
    fn to_record_excludes_content() {
        let result = BrowserSearchResult {
            url: "https://example.com".into(),
            title: "Example".into(),
            content: "lots of HTML".into(),
            rendered: true,
        };
        let rec = result.to_record();
        assert!(rec.get("content").is_none());
        assert_eq!(rec["content_length"], 12);
        assert_eq!(rec["source"], "browser_fallback");
    }

    #[test]
    fn to_content_record_includes_content() {
        let result = BrowserSearchResult {
            url: "https://example.com".into(),
            title: "Example".into(),
            content: "page text".into(),
            rendered: true,
        };
        let rec = result.to_content_record();
        assert_eq!(rec["content"], "page text");
    }

    #[test]
    fn extract_json_blocks_finds_script_json() {
        let result = BrowserSearchResult {
            url: "https://example.com".into(),
            title: "Example".into(),
            content: r#"
                <html><body>
                <script type="application/json">{"results": [{"name": "Alice"}]}</script>
                </body></html>
            "#
            .into(),
            rendered: true,
        };
        let blocks = result.extract_json_blocks();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0]["results"].is_array());
    }

    #[test]
    fn extract_json_blocks_returns_empty_when_none_found() {
        let result = BrowserSearchResult {
            url: "https://example.com".into(),
            title: "Example".into(),
            content: "<html><body><p>No JSON here</p></body></html>".into(),
            rendered: true,
        };
        assert!(result.extract_json_blocks().is_empty());
    }

    #[test]
    fn extract_window_data_finds_preloaded_state() {
        let result = BrowserSearchResult {
            url: "https://example.com".into(),
            title: "SPA".into(),
            content: r#"<script>window.__PRELOADED_STATE__ = {"items": [1, 2, 3]};</script>"#
                .into(),
            rendered: true,
        };
        let data = result.extract_window_data();
        assert_eq!(data.len(), 1);
        assert!(data[0]["items"].is_array());
    }
}
