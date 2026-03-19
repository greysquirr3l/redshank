//! `GetWikiEntry` query and handler.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::domain::auth::{AuthContext, StaticPolicy, can_read_wiki};
use crate::domain::errors::DomainError;
use crate::domain::wiki::WikiEntry;
use crate::ports::wiki_store::WikiStore;

/// Query to retrieve a wiki entry by path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetWikiEntryQuery {
    /// Path to the wiki entry (used to derive the title via the file stem).
    pub path: PathBuf,
    /// Caller's auth context.
    pub auth: AuthContext,
}

/// Handles the [`GetWikiEntryQuery`].
///
/// Enforces `ReadWiki` permission, derives the entry title from the path's
/// file stem, and delegates to the [`WikiStore`] port.
pub struct GetWikiEntryHandler<W> {
    wiki_store: W,
    policy: StaticPolicy,
}

impl<W: WikiStore> GetWikiEntryHandler<W> {
    /// Create a new handler backed by the given wiki store.
    #[must_use]
    pub const fn new(wiki_store: W) -> Self {
        Self {
            wiki_store,
            policy: StaticPolicy,
        }
    }

    /// Execute the query.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::Security`] if the caller lacks `ReadWiki`
    /// permission, or a storage error if the lookup fails.
    pub async fn handle(&self, query: GetWikiEntryQuery) -> Result<Option<WikiEntry>, DomainError> {
        can_read_wiki(&query.auth, &self.policy).map_err(DomainError::Security)?;

        // Derive title from the path's file stem (strip .md extension).
        let title = query
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .replace(['-', '_'], " ");

        self.wiki_store.read_entry(&title).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::domain::auth::{Role, UserId};
    use crate::domain::credentials::CredentialGuard;
    use crate::domain::wiki::{WikiCategory, WikiEntry};

    struct MockWikiStore {
        entry: Option<WikiEntry>,
    }

    impl WikiStore for MockWikiStore {
        async fn write_entry(&self, _entry: &WikiEntry) -> Result<(), DomainError> {
            Ok(())
        }

        async fn read_entry(&self, _title: &str) -> Result<Option<WikiEntry>, DomainError> {
            Ok(self.entry.clone())
        }

        async fn list_entries(
            &self,
            _category: Option<&crate::domain::wiki::WikiCategory>,
        ) -> Result<Vec<WikiEntry>, DomainError> {
            Ok(self.entry.iter().cloned().collect())
        }
    }

    fn owner_auth() -> AuthContext {
        AuthContext::owner(UserId::new(), "tok".into())
    }

    fn reader_auth() -> AuthContext {
        AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Reader],
            session_token: CredentialGuard::new("tok".into()),
        }
    }

    #[tokio::test]
    async fn handler_returns_entry_for_owner() {
        let entry = WikiEntry {
            path: PathBuf::from("corporate/acme-corp.md"),
            title: "acme corp".to_string(),
            category: WikiCategory::Corporate,
            cross_refs: vec![],
        };
        let handler = GetWikiEntryHandler::new(MockWikiStore {
            entry: Some(entry.clone()),
        });
        let result = handler
            .handle(GetWikiEntryQuery {
                path: PathBuf::from("corporate/acme-corp.md"),
                auth: owner_auth(),
            })
            .await
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn handler_denies_non_reader() {
        // A user with no roles (empty) should be denied.
        let no_role_auth = AuthContext {
            user_id: UserId::new(),
            roles: vec![],
            session_token: CredentialGuard::new("tok".into()),
        };
        let handler = GetWikiEntryHandler::new(MockWikiStore { entry: None });
        let result = handler
            .handle(GetWikiEntryQuery {
                path: PathBuf::from("any.md"),
                auth: no_role_auth,
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn handler_allows_reader() {
        let handler = GetWikiEntryHandler::new(MockWikiStore { entry: None });
        let result = handler
            .handle(GetWikiEntryQuery {
                path: PathBuf::from("any.md"),
                auth: reader_auth(),
            })
            .await;
        assert!(result.is_ok());
    }
}
