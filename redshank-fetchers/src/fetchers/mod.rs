//! Individual fetcher implementations.
//!
//! Each submodule implements a public data source fetcher as a library,
//! with a corresponding binary in `src/bin/` for CLI invocation.

// ── T19: 12 ported fetchers ──────────────────────────────────────────────────
pub mod census_acs;
pub mod epa_echo;
pub mod fdic;
pub mod fec;
pub mod icij_leaks;
pub mod ofac_sdn;
pub mod osha;
pub mod propublica_990;
pub mod sam_gov;
pub mod sec_edgar;
pub mod senate_lobbying;
pub mod usaspending;

// ── T20: 14 extended fetchers ────────────────────────────────────────────────
pub mod county_property;
pub mod courtlistener;
pub mod eu_sanctions;
pub mod federal_audit;
pub mod fincen_boi;
pub mod fpds;
pub mod gdelt;
pub mod gleif;
pub mod house_lobbying;
pub mod opencorporates;
pub mod state_sos;
pub mod un_sanctions;
pub mod wikidata;
pub mod world_bank_debarred;

// ── T21: 8 individual-person OSINT fetchers ──────────────────────────────────
pub mod github_profile;
pub mod hibp;
pub mod social_profiles;
pub mod username_enum;
pub mod uspto;
pub mod voter_reg;
pub mod wayback;
pub mod whois_rdap;

// ── T27: 9 regulatory enforcement fetchers ───────────────────────────────────
pub mod cfpb;
pub mod cftc;
pub mod fda_warnings;
pub mod ftc;
pub mod gsa_eoffer;
pub mod msha;
pub mod nhtsa;
pub mod nlrb;
pub mod npi;

// ── T28: FARA and FINRA fetchers ─────────────────────────────────────────────
pub mod fara;
pub mod finra_brokercheck;

// ── T29: International corporate registry and sanctions fetchers ──────────────
pub mod australia_dfat_sanctions;
pub mod canada_corporations;
pub mod canada_sema_sanctions;
pub mod opensanctions;
pub mod uk_companies_house;
pub mod uk_hmt_sanctions;
