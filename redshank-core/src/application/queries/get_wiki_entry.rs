//! `GetWikiEntry` query.

use serde::{Deserialize, Serialize};

/// Query to retrieve a wiki entry by title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWikiEntryQuery {
    /// Entry title to look up.
    pub title: String,
}

// TODO(T16): Implement GetWikiEntryHandler
