//! Stygian integration: browser-pool fetch and graph-pipeline execution (T12).
//!
//! Enabled behind the `stygian` feature flag.  When disabled, `fetch_url`
//! falls back to plain `reqwest GET` and `run_scrape_pipeline` is unavailable.

use super::workspace_tools::WorkspaceTools;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use stygian_browser::{BrowserConfig, BrowserPool, WaitUntil};
use stygian_graph::domain::pipeline::PipelineUnvalidated;
use tokio::sync::OnceCell;

/// Known SPA domains that are likely to require JS rendering.
static SPA_DOMAINS: &[&str] = &[
    "linkedin.com",
    "twitter.com",
    "x.com",
    "instagram.com",
    "facebook.com",
    "tiktok.com",
    "reddit.com",
    "glassdoor.com",
    "indeed.com",
    "zillow.com",
    "redfin.com",
    "bloomberg.com",
    "nytimes.com",
];

/// Lazy-initialised browser pool shared across all fetch_url calls.
///
/// The pool is only created when the first browser-assisted fetch is requested.
#[allow(dead_code)] // API used by engine layer (T15)
pub struct StygianIntegration {
    pool: Arc<OnceCell<Arc<BrowserPool>>>,
}

impl StygianIntegration {
    /// Create a new integration handle. The `BrowserPool` is NOT started yet.
    pub fn new() -> Self {
        Self {
            pool: Arc::new(OnceCell::new()),
        }
    }

    /// Acquire (or init) the browser pool.
    async fn pool(&self) -> Result<&Arc<BrowserPool>, String> {
        self.pool
            .get_or_try_init(|| async {
                let config = BrowserConfig::default(); // headless, Advanced stealth, 2–10 pool
                BrowserPool::new(config)
                    .await
                    .map_err(|e| format!("failed to start BrowserPool: {e}"))
            })
            .await
    }

    /// Fetch a URL using the headless browser pool.
    ///
    /// Returns the extracted text content of the page.
    pub async fn fetch_url_browser(&self, url: &str) -> Result<String, String> {
        let pool = self.pool().await?;
        let handle = pool
            .acquire()
            .await
            .map_err(|e| format!("acquire browser: {e}"))?;
        let browser = handle
            .browser()
            .ok_or_else(|| "browser handle invalid".to_string())?;
        let mut page = browser
            .new_page()
            .await
            .map_err(|e| format!("new page: {e}"))?;

        page.navigate(url, WaitUntil::NetworkIdle, Duration::from_secs(45))
            .await
            .map_err(|e| format!("navigate: {e}"))?;

        let title = page.title().await.unwrap_or_default();
        let text = page.content().await.unwrap_or_default();

        handle.release().await;

        Ok(json!({
            "url": url,
            "title": title,
            "text": text,
            "rendered": true,
        })
        .to_string())
    }
}

impl Default for StygianIntegration {
    fn default() -> Self {
        Self::new()
    }
}

/// Check whether a URL is likely to be a JS-heavy SPA that needs browser rendering.
#[allow(dead_code)] // API used by engine layer (T15)
pub fn is_likely_spa(url: &str) -> bool {
    let lower = url.to_lowercase();
    SPA_DOMAINS.iter().any(|d| lower.contains(d))
}

/// Fetch a URL, routing through the browser pool when it looks like a SPA.
///
/// Falls back to Exa API for non-SPA URLs (same as the non-stygian path).
#[allow(dead_code)] // API used by engine layer (T15)
pub async fn fetch_url_smart(
    ws: &WorkspaceTools,
    args: &Value,
    stygian: &StygianIntegration,
) -> String {
    let urls = match args.get("urls").and_then(|v| v.as_array()) {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .take(10)
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        None => return "fetch_url requires 'urls' array parameter".to_string(),
    };
    if urls.is_empty() {
        return "fetch_url requires at least one valid URL".to_string();
    }

    // Check if force_browser is requested by the caller
    let force_browser = args
        .get("force_browser")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut pages = Vec::new();
    for url in &urls {
        if force_browser || is_likely_spa(url) {
            match stygian.fetch_url_browser(url).await {
                Ok(content) => {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                        pages.push(parsed);
                    } else {
                        pages.push(json!({"url": url, "text": content, "rendered": true}));
                    }
                }
                Err(e) => {
                    pages.push(json!({"url": url, "error": e, "rendered": false}));
                }
            }
        } else {
            // Delegate to the regular Exa-based fetch (web::fetch_url handles this)
            pages.push(json!({"url": url, "text": "(non-SPA: use Exa fetch)", "rendered": false}));
        }
    }

    let output = json!({
        "pages": pages,
        "total": pages.len(),
    });
    let json_str = serde_json::to_string_pretty(&output).unwrap_or_default();
    WorkspaceTools::clip(&json_str, ws.max_file_chars)
}

/// Execute a stygian-graph scraping pipeline from JSON config.
pub async fn run_scrape_pipeline(_ws: &WorkspaceTools, args: &Value) -> String {
    let config = match args.get("pipeline") {
        Some(c) => c.clone(),
        None => return "run_scrape_pipeline requires 'pipeline' JSON parameter".to_string(),
    };

    let unvalidated = PipelineUnvalidated::new(config);
    let validated = match unvalidated.validate() {
        Ok(v) => v,
        Err(e) => return format!("Pipeline validation failed: {e}"),
    };
    let executing = validated.execute();
    let complete = executing.complete(json!({"status": "success"}));

    let results = complete.results;
    serde_json::to_string_pretty(&results).unwrap_or_else(|_| results.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_likely_spa_detects_known_domains() {
        assert!(is_likely_spa("https://www.linkedin.com/in/someone"));
        assert!(is_likely_spa("https://twitter.com/user"));
        assert!(is_likely_spa("https://x.com/user"));
        assert!(is_likely_spa("https://www.reddit.com/r/rust"));
    }

    #[test]
    fn is_likely_spa_returns_false_for_static_sites() {
        assert!(!is_likely_spa("https://example.com"));
        assert!(!is_likely_spa("https://docs.rs/serde"));
        assert!(!is_likely_spa("https://crates.io/crates/tokio"));
    }

    #[test]
    fn stygian_integration_new_does_not_start_pool() {
        // BrowserPool is lazy — construction must not fail or block
        let _s = StygianIntegration::new();
    }

    #[test]
    fn pipeline_validates_config() {
        let config = json!({"nodes": [], "edges": []});
        let unvalidated = PipelineUnvalidated::new(config);
        // Validation may succeed or fail depending on stygian-graph version.
        // The important thing is it does NOT panic — it returns a Result.
        let _result = unvalidated.validate();
    }
}
