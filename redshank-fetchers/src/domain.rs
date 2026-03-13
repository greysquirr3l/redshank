//! Fetcher domain types.

use serde::{Deserialize, Serialize};

/// Configuration for a fetcher invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchConfig {
    /// Data source name.
    pub source: String,
    /// Query string.
    pub query: String,
    /// Maximum results to return.
    pub max_results: Option<u32>,
}

/// Output record from a fetcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchOutput {
    /// Source identifier.
    pub source: String,
    /// The data payload (NDJSON-compatible).
    pub data: serde_json::Value,
}
