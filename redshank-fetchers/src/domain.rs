//! Fetcher domain types and shared infrastructure.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ── FetchConfig ─────────────────────────────────────────────

/// Configuration for a paginated fetcher invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchConfig {
    /// Base URL of the API endpoint.
    pub base_url: String,
    /// Query parameters (merged into each request).
    #[serde(default)]
    pub query_params: HashMap<String, String>,
    /// Extra HTTP headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Delay in milliseconds between paginated requests (rate limiting).
    #[serde(default = "default_rate_limit_ms")]
    pub rate_limit_ms: u64,
    /// Maximum number of pages to fetch (0 = unlimited).
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    /// Directory where output NDJSON files are written.
    pub output_path: PathBuf,
}

const fn default_rate_limit_ms() -> u64 {
    500
}

const fn default_max_pages() -> u32 {
    100
}

impl FetchConfig {
    /// Validate that the output directory's parent exists.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the parent directory of `output_path` does not exist.
    pub fn validate(&self) -> Result<(), FetchError> {
        if let Some(parent) = self.output_path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            return Err(FetchError::InvalidOutputPath {
                path: self.output_path.clone(),
                reason: format!("parent directory does not exist: {}", parent.display()),
            });
        }
        Ok(())
    }
}

// ── Attribution ─────────────────────────────────────────────

/// `ODbL` / licence attribution that **must** appear in any report or
/// user-facing output that surfaces data from the originating source.
///
/// The `text` field is the human-readable attribution line.
/// The `url` field is the hyperlink target that `text` must link to.
/// The `min_font_size_px` constraint is derived from the `ODbL`:
/// attribution must be ≥ 70 % of the largest related font, and never
/// smaller than 7 px — encode whichever is larger here.
///
/// All fields are required; omitting any means the licence is not
/// correctly satisfied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribution {
    /// Short label identifying the source (e.g. `"OpenCorporates"`).
    pub source: String,
    /// Human-readable attribution text that must be displayed.
    pub text: String,
    /// URL the `text` must hyperlink to (homepage or canonical entity page).
    pub url: String,
    /// Minimum display size in pixels required by the originating licence.
    pub min_font_size_px: u8,
    /// SPDX licence identifier (e.g. `"ODbL-1.0"`).
    pub licence: String,
}

// ── FetchOutput ─────────────────────────────────────────────

/// Result of a completed fetch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchOutput {
    /// Number of records written.
    pub records_written: usize,
    /// Path to the output file.
    pub output_path: PathBuf,
    /// Human-readable source name.
    pub source_name: String,
    /// Licence attribution that **must** be included in any report or
    /// user-facing output derived from this data. `None` for sources
    /// that carry no specific attribution requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<Attribution>,
}

// ── FetchError ──────────────────────────────────────────────

/// Errors produced by data fetchers.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("invalid output path {path}: {reason}")]
    InvalidOutputPath { path: PathBuf, reason: String },

    #[error("API error: {status} — {body}")]
    ApiError { status: u16, body: String },

    #[error("parse error: {0}")]
    Parse(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn default_config(output_path: PathBuf) -> FetchConfig {
        FetchConfig {
            base_url: "https://api.example.com".into(),
            query_params: HashMap::new(),
            headers: HashMap::new(),
            rate_limit_ms: 500,
            max_pages: 10,
            output_path,
        }
    }

    #[test]
    fn validate_accepts_existing_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let config = default_config(dir.path().join("output.ndjson"));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_nonexistent_parent() {
        let config = default_config(PathBuf::from("/no/such/parent/output.ndjson"));
        let err = config.validate().unwrap_err();
        assert!(matches!(err, FetchError::InvalidOutputPath { .. }));
    }

    #[test]
    fn validate_accepts_relative_path_with_empty_parent() {
        // A simple filename like "output.ndjson" has parent "" which should be accepted.
        let config = default_config(PathBuf::from("output.ndjson"));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn fetch_output_serialises_to_json() {
        let output = FetchOutput {
            records_written: 42,
            output_path: PathBuf::from("/data/output.ndjson"),
            source_name: "fec".into(),
            attribution: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"records_written\":42"));
        assert!(json.contains("\"source_name\":\"fec\""));
        // attribution: None is skip_serializing, so it must be absent
        assert!(!json.contains("attribution"));
    }

    #[test]
    fn attribution_roundtrips_serde() {
        let attr = Attribution {
            source: "OpenCorporates".into(),
            text: "from OpenCorporates".into(),
            url: "https://opencorporates.com".into(),
            min_font_size_px: 7,
            licence: "ODbL-1.0".into(),
        };
        let json = serde_json::to_string(&attr).unwrap();
        let restored: Attribution = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, attr);
        assert!(json.contains("ODbL-1.0"));
    }
}
