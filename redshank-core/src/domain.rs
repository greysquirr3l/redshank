//! Pure domain layer — zero I/O dependencies.
//!
//! Contains aggregates, value objects, domain events, and error types.
//! No module in this subtree may depend on tokio, reqwest, rusqlite,
//! or any other I/O crate.

pub mod agent;
pub mod auth;
pub mod credentials;
pub mod errors;
pub mod events;
pub mod observation;
pub mod session;
pub mod settings;
pub mod source_catalog;
pub mod temporal;
pub mod wiki;
