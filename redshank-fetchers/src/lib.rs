//! # redshank-fetchers
//!
//! Public data fetcher binaries and shared infrastructure for the
//! Redshank investigation agent.
//!
//! Each fetcher is a standalone binary in `src/bin/` accepting `--output <dir>`
//! and `--query <string>` CLI arguments. All output is newline-delimited JSON
//! (NDJSON) for agent consumption.

pub mod client;
pub mod domain;

// Re-export commonly used types.
pub use client::{build_client, build_client_from_config, rate_limit_delay, write_ndjson};
pub use domain::{FetchConfig, FetchError, FetchOutput};

// ── Shared CLI argument parsing ─────────────────────────────

/// Common CLI arguments shared by all fetcher binaries.
#[derive(Debug, clap::Parser)]
pub struct FetcherArgs {
    /// Search query or entity name.
    #[arg(short, long)]
    pub query: String,

    /// Output directory for NDJSON files.
    #[arg(short, long, default_value = ".")]
    pub output: std::path::PathBuf,

    /// API key (if required by the source).
    #[arg(long, env)]
    pub api_key: Option<String>,

    /// Rate limit in milliseconds between paginated requests.
    #[arg(long, default_value_t = 500)]
    pub rate_limit_ms: u64,

    /// Maximum pages to fetch (0 = unlimited).
    #[arg(long, default_value_t = 100)]
    pub max_pages: u32,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {
        assert!(true);
    }
}
