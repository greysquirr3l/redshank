//! `DataFetcher` port — public data source fetching.

use crate::domain::errors::DomainError;
use serde_json::Value;

/// Port trait for data fetcher implementations.
pub trait DataFetcher: Send + Sync {
    /// Human-readable name of the data source.
    fn source_name(&self) -> &str;

    /// Fetch data for the given query and return NDJSON-compatible records.
    fn fetch(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Value>, DomainError>> + Send;
}
