//! `GetWikiEntry` query.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::domain::auth::AuthContext;

/// Query to retrieve a wiki entry by path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWikiEntryQuery {
    /// Path to the wiki entry.
    pub path: PathBuf,
    /// Caller's auth context.
    pub auth: AuthContext,
}

// TODO(T16): Implement GetWikiEntryHandler
