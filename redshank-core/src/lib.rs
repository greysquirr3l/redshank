//! # redshank-core
//!
//! Core library for the Redshank investigation agent. Follows hexagonal
//! architecture (ports & adapters) with DDD-Lite aggregates, CQRS
//! command/query separation, and security-first repository design.
//!
//! ## Module layout (mirrors stygian-graph)
//!
//! - [`domain`] — Pure types, aggregates, value objects, domain events. Zero I/O.
//! - [`ports`] — Trait interfaces (inbound + outbound). Reference only domain types.
//! - [`application`] — CQRS command/query handlers and orchestration services.
//! - [`adapters`] — I/O implementations: LLM providers, tools, persistence.

pub mod domain;
pub mod ports;

#[cfg(feature = "runtime")]
pub mod application;

#[cfg(feature = "runtime")]
pub mod adapters;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {
        // Smoke test: this crate compiles successfully.
    }
}
