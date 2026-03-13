//! `WikiStore` port — wiki entry persistence.

use crate::domain::errors::DomainError;
use crate::domain::wiki::{WikiCategory, WikiEntry};

/// Port trait for wiki storage.
pub trait WikiStore: Send + Sync {
    /// Write or update a wiki entry.
    fn write_entry(
        &self,
        entry: &WikiEntry,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send;

    /// Read a wiki entry by title.
    fn read_entry(
        &self,
        title: &str,
    ) -> impl std::future::Future<Output = Result<Option<WikiEntry>, DomainError>> + Send;

    /// List all wiki entries, optionally filtered by category.
    fn list_entries(
        &self,
        category: Option<&WikiCategory>,
    ) -> impl std::future::Future<Output = Result<Vec<WikiEntry>, DomainError>> + Send;
}
