//! Individual fetcher implementations.
//!
//! Each submodule implements a public data source fetcher as a library,
//! with a corresponding binary in `src/bin/` for CLI invocation.

// ── T19: 12 ported fetchers ──────────────────────────────────────────────────
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

// ── T20: 14 extended fetchers ────────────────────────────────────────────────
pub mod fincen_boi;
pub mod gleif;
pub mod opencorporates;
pub mod house_lobbying;
pub mod courtlistener;
pub mod un_sanctions;
pub mod eu_sanctions;
pub mod world_bank_debarred;
pub mod federal_audit;
pub mod fpds;
pub mod wikidata;
pub mod gdelt;
pub mod state_sos;
pub mod county_property;

// ── T21: 8 individual-person OSINT fetchers ──────────────────────────────────
pub mod hibp;
pub mod github_profile;
pub mod wayback;
pub mod whois_rdap;
pub mod voter_reg;
pub mod uspto;
pub mod username_enum;
pub mod social_profiles;
