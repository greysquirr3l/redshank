//! Individual fetcher implementations.
//!
//! Each submodule implements a public data source fetcher as a library,
//! with a corresponding binary in `src/bin/` for CLI invocation.

// ── T19: 12 ported fetchers ──────────────────────────────────────────────────
pub mod bls_qcew;
pub mod census_acs;
pub mod clinical_trials;
pub mod cms_open_payments;
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
pub mod amazon_authors;
pub mod county_property;
pub mod courtlistener;
pub mod eu_sanctions;
pub mod federal_audit;
pub mod fincen_boi;
pub mod fpds;
pub mod gdelt;
pub mod gleif;
pub mod google_scholar;
pub mod house_lobbying;
pub mod opencorporates;
pub mod pacer;
pub mod state_sos;
pub mod un_sanctions;
pub mod wikidata;
pub mod world_bank_debarred;

// ── T38: Nonprofit and IRS intelligence fetchers ────────────────────────────
pub mod guidestar_candid;
pub mod irs_1023;
pub mod irs_990_xml;

// ── T39: Crypto and alternative finance fetchers ────────────────────────────
pub mod blockchain_explorer;
pub mod defi_protocols;
pub mod exchange_transparency;
pub mod tornado_screening;

// ── T40: Environmental and permits intelligence fetchers ────────────────────
pub mod carbon_registries;
pub mod epa_superfund;
pub mod sec_climate;
pub mod state_env_permits;

// ── T41: EU business register fetchers ───────────────────────────────────────
pub mod eu_bris;
pub mod france_infogreffe;
pub mod germany_handelsregister;

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
pub mod crunchbase;
pub mod fda_warnings;
pub mod ftc;
pub mod gsa_eoffer;
pub mod msha;
pub mod nhtsa;
pub mod nlrb;
pub mod npi;
pub mod npi_extended;

// ── T28: FARA and FINRA fetchers ─────────────────────────────────────────────
pub mod fara;
pub mod finra_brokercheck;

// ── T29: International corporate registry and sanctions fetchers ──────────────
pub mod australia_dfat_sanctions;
pub mod canada_corporations;
pub mod canada_sema_sanctions;
pub mod opensanctions;
pub mod uk_companies_house;
pub mod uk_corporate_intelligence;
pub mod uk_hmt_sanctions;

// ── T30: Aviation and maritime asset intelligence fetchers ────────────────────
pub mod faa_nnumber;
pub mod maritime_ais;

// ── T31: UCC filings and property intelligence fetchers ───────────────────────
pub mod assessor_portals;
pub mod delaware_franchise_tax;
pub mod property_valuation;
pub mod sec_13d_13g;
pub mod ucc_filings;

// ── T32: Academic and media intelligence fetchers ─────────────────────────────
pub mod bluesky;
pub mod common_crawl;
pub mod hackernews;
pub mod linkedin_public;
pub mod listen_notes;
pub mod mastodon;
pub mod orcid;
pub mod reddit;
pub mod sec_xbrl;
pub mod semantic_scholar;
pub mod youtube;
