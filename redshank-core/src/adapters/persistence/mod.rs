//! Persistence adapters.

pub mod credential_store;
pub mod replay_log;
pub mod settings_store;

// TODO(T17): sqlite.rs — SqliteSessionStore implements SessionStore
// TODO(T16): wiki_fs.rs — FsWikiStore implements WikiStore
