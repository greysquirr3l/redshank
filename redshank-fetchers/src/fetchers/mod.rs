//! Individual fetcher implementations.
//!
//! Each submodule implements a public data source fetcher as a library,
//! with a corresponding binary in `src/bin/` for CLI invocation.

pub mod fec;
pub mod sec_edgar;
pub mod usaspending;
pub mod senate_lobbying;
pub mod ofac_sdn;
pub mod icij_leaks;
pub mod propublica_990;
pub mod census_acs;
pub mod epa_echo;
pub mod fdic;
pub mod osha;
pub mod sam_gov;
