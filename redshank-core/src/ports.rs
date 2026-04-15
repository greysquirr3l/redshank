//! Port interfaces (inbound + outbound).
//!
//! Traits reference only domain types. Adapters implement these traits.
//! All traits are object-safe.

pub mod fetcher;
pub mod model_provider;
pub mod observation_store;
pub mod replay_log;
pub mod session_store;
pub mod tool_dispatcher;
pub mod wiki_store;
pub mod workspace_config;
