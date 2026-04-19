//! Source catalog metadata: typed registry of all data fetchers with categories,
//! access requirements, and help text.

use serde::{Deserialize, Serialize};

/// Source data category classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceCategory {
    /// Corporate registries, business info, company structure.
    Corporate,
    /// Sanctions lists and designations (OFAC, UN, EU, World Bank, etc.).
    Sanctions,
    /// Federal and state courts, case filings, dockets.
    Courts,
    /// Social media, video, news aggregation, media archives.
    Media,
    /// Academic publications, researcher profiles, papers.
    Academic,
    /// Blockchain, `DeFi`, crypto exchanges, transaction analysis.
    Crypto,
    /// Nonprofit filings, `990s`, `GuideStar`, charitable registries.
    Nonprofit,
    /// Financial regulators, healthcare, labor, environmental, consumer protection.
    Regulatory,
    /// Environmental permits, superfund sites, carbon registries, climate disclosures.
    Environmental,
    /// Open-source intelligence: breaches, usernames, WHOIS, GitHub, USPTO, voter rolls.
    Osint,
    /// Government contracts, spending, lobbying, federal records.
    Government,
}

impl SourceCategory {
    /// Display name for the category.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Corporate => "Corporate Registries",
            Self::Sanctions => "Sanctions Lists",
            Self::Courts => "Courts & Legal",
            Self::Media => "Media & Archives",
            Self::Academic => "Academic & Research",
            Self::Crypto => "Cryptocurrency",
            Self::Nonprofit => "Nonprofits & Charities",
            Self::Regulatory => "Regulatory & Compliance",
            Self::Environmental => "Environmental",
            Self::Osint => "Open-Source Intelligence",
            Self::Government => "Government & Contracts",
        }
    }
}

/// Access requirement for a data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthRequirement {
    /// Public access, no authentication required.
    None,
    /// Optional authentication for increased rate limits.
    Optional,
    /// API key or credentials required to access.
    Required,
}

impl AuthRequirement {
    /// Display name for the auth requirement.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::None => "Public",
            Self::Optional => "Optional",
            Self::Required => "Required",
        }
    }
}

/// Metadata for a single data source fetcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDescriptor {
    /// Unique source ID (lowercase `snake_case`, e.g., `"opencorporates"`, `"fec"`).
    pub id: &'static str,
    /// Display name for the source.
    pub title: &'static str,
    /// Short description of the data returned by this source.
    pub description: &'static str,
    /// Source category.
    pub category: SourceCategory,
    /// Homepage or main URL for the source.
    pub homepage_url: &'static str,
    /// Access requirement (public, optional API key, required).
    pub auth_requirement: AuthRequirement,
    /// Field name in `credentials.json` if credentials are needed (e.g., `"opencorporates_api_key"`).
    pub credential_field: Option<&'static str>,
    /// Whether this source is enabled by default.
    pub enabled_by_default: bool,
    /// Access instructions or sign-up URL.
    pub access_instructions: &'static str,
}

/// Get a source descriptor by ID.
#[must_use]
pub fn source_by_id(id: &str) -> Option<&'static SourceDescriptor> {
    SOURCES.iter().find(|s| s.id == id)
}

/// Get all sources in a category, sorted by title.
#[must_use]
pub fn sources_by_category(category: SourceCategory) -> Vec<&'static SourceDescriptor> {
    let mut sources: Vec<_> = SOURCES.iter().filter(|s| s.category == category).collect();
    sources.sort_by(|a, b| a.title.cmp(b.title));
    sources
}

/// Get all sources, optionally filtered by enabled status, sorted by category then title.
#[must_use]
pub fn all_sources(enabled_only: bool) -> Vec<&'static SourceDescriptor> {
    let mut sources: Vec<_> = SOURCES
        .iter()
        .filter(|s| !enabled_only || s.enabled_by_default)
        .collect();
    sources.sort_by(|a, b| {
        a.category
            .display_name()
            .cmp(b.category.display_name())
            .then(a.title.cmp(b.title))
    });
    sources
}

/// Get all source IDs.
#[must_use]
pub fn all_source_ids() -> Vec<&'static str> {
    SOURCES.iter().map(|s| s.id).collect()
}

/// Static registry of all known data sources.
pub static SOURCES: &[SourceDescriptor] = &[
    // ────── T19: Government & Regulatory (Core 12+) ──────────────────────────

    // FEC - Federal Election Commission campaign finance
    SourceDescriptor {
        id: "fec",
        title: "FEC Campaign Finance",
        description: "U.S. Federal Election Commission campaign contributions, expenditures, and candidate finance disclosures.",
        category: SourceCategory::Government,
        homepage_url: "https://www.fec.gov/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: Some("fec_api_key"),
        enabled_by_default: true,
        access_instructions: "API key optional; https://api.open.fec.gov/",
    },
    // SEC EDGAR - Securities Exchange Commission filings
    SourceDescriptor {
        id: "sec_edgar",
        title: "SEC EDGAR",
        description: "U.S. Securities and Exchange Commission filings: 10-K, 10-Q, 8-K, S-1, proxy statements.",
        category: SourceCategory::Government,
        homepage_url: "https://www.sec.gov/cgi-bin/browse-edgar",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public access; no authentication required.",
    },
    // Senate Lobbying Records
    SourceDescriptor {
        id: "senate_lobbying",
        title: "Senate Lobbying Disclosures",
        description: "U.S. Senate Lobbying Disclosure Act filings: lobbyists, clients, spending, issues.",
        category: SourceCategory::Government,
        homepage_url: "https://soprweb.senate.gov/index.cfm?action=home",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; no authentication required.",
    },
    // House Lobbying Records
    SourceDescriptor {
        id: "house_lobbying",
        title: "House Lobbying Disclosures",
        description: "U.S. House Clerk Lobbying Disclosure Act filings.",
        category: SourceCategory::Government,
        homepage_url: "https://lobbyingdisclosure.house.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; downloadable XML files.",
    },
    // OFAC SDN - Sanctions list
    SourceDescriptor {
        id: "ofac_sdn",
        title: "OFAC SDN List",
        description: "U.S. Treasury OFAC Specially Designated Nationals and blocked persons list.",
        category: SourceCategory::Sanctions,
        homepage_url: "https://sanctionsearch.ofac.treas.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public access; downloadable CSV/XML from OFAC website.",
    },
    // UN Sanctions
    SourceDescriptor {
        id: "un_sanctions",
        title: "UN Sanctions Lists",
        description: "United Nations Security Council consolidated sanctions lists.",
        category: SourceCategory::Sanctions,
        homepage_url: "https://www.un.org/securitycouncil/sanctions/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public access; downloadable XML from UN website.",
    },
    // USA Spending
    SourceDescriptor {
        id: "usaspending",
        title: "USAspending.gov",
        description: "U.S. federal government spending, contracts, grants, and awards.",
        category: SourceCategory::Government,
        homepage_url: "https://www.usaspending.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public API; no authentication required.",
    },
    // FPDS - Federal Procurement Data System
    SourceDescriptor {
        id: "fpds",
        title: "FPDS",
        description: "Federal procurement contracts reported to the Federal Procurement Data System.",
        category: SourceCategory::Government,
        homepage_url: "https://www.fpds.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; queryable interface.",
    },
    // SAM.gov - System for Award Management
    SourceDescriptor {
        id: "sam_gov",
        title: "SAM.gov",
        description: "System for Award Management: federal contractors, entities, excluded/debarred parties.",
        category: SourceCategory::Government,
        homepage_url: "https://sam.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public API; limited rate limits without registration.",
    },
    // Census ACS - American Community Survey
    SourceDescriptor {
        id: "census_acs",
        title: "U.S. Census ACS",
        description: "American Community Survey demographic and socioeconomic data by geographic area.",
        category: SourceCategory::Government,
        homepage_url: "https://www.census.gov/acs/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "API key recommended; https://api.census.gov/data/key_signup.html",
    },
    // ICIJ Offshore Leaks
    SourceDescriptor {
        id: "icij_leaks",
        title: "ICIJ Offshore Leaks",
        description: "International Consortium of Investigative Journalists offshore entity database.",
        category: SourceCategory::Corporate,
        homepage_url: "https://offshoreleaks.icij.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; bulk data available.",
    },
    // BLS QCEW - Bureau of Labor Statistics Quarterly Census of Employment & Wages
    SourceDescriptor {
        id: "bls_qcew",
        title: "BLS QCEW",
        description: "Bureau of Labor Statistics employment and wage data by industry and region.",
        category: SourceCategory::Government,
        homepage_url: "https://www.bls.gov/cew/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; https://www.bls.gov/developers/",
    },
    // EPA ECHO - Environmental Compliance History Online
    SourceDescriptor {
        id: "epa_echo",
        title: "EPA ECHO",
        description: "EPA enforcement, compliance, and history of environmental violations.",
        category: SourceCategory::Environmental,
        homepage_url: "https://echo.epa.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public API; no authentication required.",
    },
    // FDIC Failed Banks
    SourceDescriptor {
        id: "fdic",
        title: "FDIC Failed Banks",
        description: "FDIC list of failed insured depository institutions.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.fdic.gov/resources/resolutions/bank-failures/failed-bank-list/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public data; historical failed banks list.",
    },
    // OSHA - Occupational Safety and Health Administration violations
    SourceDescriptor {
        id: "osha",
        title: "OSHA Violations",
        description: "OSHA workplace safety inspection violations and penalties.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.osha.gov/data/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public API; queryable inspection records.",
    },
    // ProPublica 990
    SourceDescriptor {
        id: "propublica_990",
        title: "ProPublica Tax-Exempt Explorer",
        description: "ProPublica nonprofit tax-exempt organization search and IRS Form 990 data.",
        category: SourceCategory::Nonprofit,
        homepage_url: "https://projects.propublica.org/nonprofits/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; web scraping or API available.",
    },
    // Clinical Trials
    SourceDescriptor {
        id: "clinical_trials",
        title: "ClinicalTrials.gov",
        description: "NIH clinical trials registry with protocol, recruitment, and results data.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://clinicaltrials.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; XML/JSON feed available.",
    },
    // CMS Open Payments
    SourceDescriptor {
        id: "cms_open_payments",
        title: "CMS Open Payments",
        description: "Centers for Medicare & Medicaid Services physician payments and industry transfers.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://openpaymentsdata.cms.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public data; downloadable CSV from CMS.",
    },
    // ────── T20: Extended Corporate & Courts ──────────────────────────────────

    // OpenCorporates
    SourceDescriptor {
        id: "opencorporates",
        title: "OpenCorporates",
        description: "OpenCorporates largest open database of companies worldwide.",
        category: SourceCategory::Corporate,
        homepage_url: "https://opencorporates.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: Some("opencorporates_api_key"),
        enabled_by_default: true,
        access_instructions: "Free basic access; API key for enhanced search; https://opencorporates.com/api",
    },
    // GLEIF - Global Legal Entity Identifier Foundation
    SourceDescriptor {
        id: "gleif",
        title: "GLEIF",
        description: "Global Legal Entity Identifier Foundation registration data for international corporations.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.gleif.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public data; bulk downloads and API available.",
    },
    // FinCEN BOI - Beneficial Ownership Information
    SourceDescriptor {
        id: "fincen_boi",
        title: "FinCEN BOI Registry",
        description: "Treasury FinCEN beneficial ownership information on business entities.",
        category: SourceCategory::Corporate,
        homepage_url: "https://boi.treasury.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Limited public access; filed reports available to authorized users.",
    },
    // State SOS - Secretary of State Corporate Records
    SourceDescriptor {
        id: "state_sos",
        title: "State SOS Registries",
        description: "State Secretary of State corporate registrations, UCC filings, and business records.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.sos.state.us/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "State-specific; most require direct portal access.",
    },
    // County Property Records
    SourceDescriptor {
        id: "county_property",
        title: "County Property Records",
        description: "County assessor and recorder property valuations, transfers, and deeds.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.county.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "County-specific; most have public online portals.",
    },
    // CourtListener / RECAP
    SourceDescriptor {
        id: "courtlistener",
        title: "CourtListener",
        description: "Free legal research database with federal and state court documents from RECAP.",
        category: SourceCategory::Courts,
        homepage_url: "https://www.courtlistener.com/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public API; bulk data download available.",
    },
    // PACER - Public Access to Court Electronic Records
    SourceDescriptor {
        id: "pacer",
        title: "PACER",
        description: "Public Access to Court Electronic Records for federal court cases.",
        category: SourceCategory::Courts,
        homepage_url: "https://www.pacer.uscourts.gov/",
        auth_requirement: AuthRequirement::Required,
        credential_field: Some("pacer_username"),
        enabled_by_default: false,
        access_instructions: "Requires PACER account and login; paid per-document access.",
    },
    // EU Sanctions
    SourceDescriptor {
        id: "eu_sanctions",
        title: "EU Sanctions Lists",
        description: "European Union consolidated sanctions lists and entity designations.",
        category: SourceCategory::Sanctions,
        homepage_url: "https://ec.europa.eu/newsroom/dae/redirection/document/54390",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public XML feeds from EU Commission.",
    },
    // Federal Audit Clearinghouse
    SourceDescriptor {
        id: "federal_audit",
        title: "Federal Audit Clearinghouse",
        description: "U.S. Government Accountability Office audit reports and findings.",
        category: SourceCategory::Government,
        homepage_url: "https://facdatabase.census.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public database; searchable interface and downloads.",
    },
    // GDELT - Global Database of Events Language and Tone
    SourceDescriptor {
        id: "gdelt",
        title: "GDELT",
        description: "Global Database of Events, Language, and Tone: news, geopolitical events, media monitoring.",
        category: SourceCategory::Media,
        homepage_url: "https://www.gdeltproject.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public data; BigQuery access and bulk downloads available.",
    },
    // Google Scholar
    SourceDescriptor {
        id: "google_scholar",
        title: "Google Scholar",
        description: "Google Scholar researcher profiles, publications, and citation metrics.",
        category: SourceCategory::Academic,
        homepage_url: "https://scholar.google.com/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public profiles; scraping allowed with rate limiting.",
    },
    // World Bank Debarred
    SourceDescriptor {
        id: "world_bank_debarred",
        title: "World Bank Debarred",
        description: "World Bank sanctioned list of firms and individuals ineligible for contracts.",
        category: SourceCategory::Sanctions,
        homepage_url: "https://www.worldbank.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; downloadable list.",
    },
    // Wikidata
    SourceDescriptor {
        id: "wikidata",
        title: "Wikidata",
        description: "Wikidata knowledge base with structured data on entities, organizations, people.",
        category: SourceCategory::Academic,
        homepage_url: "https://www.wikidata.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public SPARQL endpoint; bulk RDF/JSON available.",
    },
    // ────── T21: Individual-Person OSINT ─────────────────────────────────────

    // HIBP - Have I Been Pwned
    SourceDescriptor {
        id: "hibp",
        title: "Have I Been Pwned",
        description: "Have I Been Pwned breach database for compromised credentials.",
        category: SourceCategory::Osint,
        homepage_url: "https://haveibeenpwned.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: Some("hibp_api_key"),
        enabled_by_default: true,
        access_instructions: "API key required for automated access.",
    },
    // GitHub Profile
    SourceDescriptor {
        id: "github_profile",
        title: "GitHub",
        description: "GitHub user profiles, repositories, contributions, and public activity.",
        category: SourceCategory::Osint,
        homepage_url: "https://github.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: Some("github_token"),
        enabled_by_default: true,
        access_instructions: "GitHub token optional; higher rate limits with auth.",
    },
    // Social Profiles - Sherlock-like enumeration
    SourceDescriptor {
        id: "social_profiles",
        title: "Social Profile Enumeration",
        description: "Username enumeration across social media and web platforms.",
        category: SourceCategory::Osint,
        homepage_url: "https://github.com/sherlock-project/sherlock",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "No authentication required; rate limiting may apply.",
    },
    // Username Enumeration
    SourceDescriptor {
        id: "username_enum",
        title: "Username Enumeration",
        description: "Multi-platform username enumeration across 300+ sites.",
        category: SourceCategory::Osint,
        homepage_url: "https://github.com/sherlock-project/sherlock",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public; web scraping with polite rate limiting.",
    },
    // USPTO - U.S. Patent and Trademark Office
    SourceDescriptor {
        id: "uspto",
        title: "USPTO Patents & Trademarks",
        description: "U.S. Patent and Trademark Office patents and trademark registrations.",
        category: SourceCategory::Osint,
        homepage_url: "https://www.uspto.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; bulk patent data available.",
    },
    // Voter Registration Records
    SourceDescriptor {
        id: "voter_reg",
        title: "Voter Registration Records",
        description: "Public voter registration records from state election authorities.",
        category: SourceCategory::Osint,
        homepage_url: "https://www.sos.state.us/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "State-specific public records; available from local election officials.",
    },
    // Wayback Machine
    SourceDescriptor {
        id: "wayback",
        title: "Wayback Machine",
        description: "Internet Archive Wayback Machine historical website snapshots.",
        category: SourceCategory::Osint,
        homepage_url: "https://web.archive.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public API; rate limiting recommended.",
    },
    // WHOIS / RDAP
    SourceDescriptor {
        id: "whois_rdap",
        title: "WHOIS/RDAP",
        description: "Domain registration and RDAP data from domain registrars.",
        category: SourceCategory::Osint,
        homepage_url: "https://www.icann.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public WHOIS and RDAP lookups; rate limits apply.",
    },
    // Stack Exchange Profiles
    SourceDescriptor {
        id: "stackexchange_profile",
        title: "Stack Exchange Profiles",
        description: "Public Stack Overflow/Stack Exchange user profiles, reputation, and activity metadata.",
        category: SourceCategory::Osint,
        homepage_url: "https://api.stackexchange.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API available without key; optional key increases quota.",
    },
    // GitLab Profiles
    SourceDescriptor {
        id: "gitlab_profile",
        title: "GitLab Profiles",
        description: "GitLab public user profiles, metadata, and account discovery by search query.",
        category: SourceCategory::Osint,
        homepage_url: "https://docs.gitlab.com/ee/api/users.html",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API available without token; token increases rate limits and scope.",
    },
    // Reverse Phone (Basic)
    SourceDescriptor {
        id: "reverse_phone_basic",
        title: "Reverse Phone (Basic)",
        description: "Best-effort phone normalization and public metadata hints without paid identity APIs.",
        category: SourceCategory::Osint,
        homepage_url: "https://www.itu.int/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "No credential required; provides metadata hints only, not subscriber identity.",
    },
    // Reverse Address (Public)
    SourceDescriptor {
        id: "reverse_address_public",
        title: "Reverse Address (Public)",
        description: "Public geocoding and address normalization using free U.S. Census geocoder endpoints.",
        category: SourceCategory::Osint,
        homepage_url: "https://geocoding.geo.census.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "No credential required; public Census geocoder endpoint.",
    },
    // ────── T27: Regulatory Enforcement ──────────────────────────────────────

    // CFPB - Consumer Financial Protection Bureau
    SourceDescriptor {
        id: "cfpb",
        title: "CFPB Consumer Complaints",
        description: "CFPB consumer complaint database for financial services.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.consumerfinance.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API and data downloads.",
    },
    // CFTC - Commodity Futures Trading Commission
    SourceDescriptor {
        id: "cftc",
        title: "CFTC Enforcement Actions",
        description: "CFTC commodity and derivatives enforcement actions and sanctions.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.cftc.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public database; searchable enforcement actions.",
    },
    // Crunchbase
    SourceDescriptor {
        id: "crunchbase",
        title: "Crunchbase",
        description: "Crunchbase startup and investor database with funding and exit information.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.crunchbase.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Freemium; paid API access for real-time data.",
    },
    // FDA Warnings
    SourceDescriptor {
        id: "fda_warnings",
        title: "FDA Warnings & Enforcement",
        description: "FDA warning letters, Class I recalls, and enforcement actions.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.fda.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public FDA database and enforcement records.",
    },
    // FTC - Federal Trade Commission
    SourceDescriptor {
        id: "ftc",
        title: "FTC Enforcement Actions",
        description: "FTC consumer protection and competition enforcement cases.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.ftc.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public FTC case database.",
    },
    // GSA eOffer - General Services Administration
    SourceDescriptor {
        id: "gsa_eoffer",
        title: "GSA eOffer",
        description: "General Services Administration contract vehicles and blanket purchase agreements.",
        category: SourceCategory::Government,
        homepage_url: "https://www.gsa.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public GSA scheduled contract listings.",
    },
    // MSHA - Mine Safety and Health Administration
    SourceDescriptor {
        id: "msha",
        title: "MSHA Inspections",
        description: "MSHA mining inspections, violations, and accident reports.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.msha.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public mine safety data.",
    },
    // NHTSA - National Highway Traffic Safety Administration
    SourceDescriptor {
        id: "nhtsa",
        title: "NHTSA Complaints",
        description: "NHTSA vehicle complaint and safety defect database.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.nhtsa.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; complaint data searchable.",
    },
    // NLRB - National Labor Relations Board
    SourceDescriptor {
        id: "nlrb",
        title: "NLRB Cases",
        description: "National Labor Relations Board labor disputes and unfair practice charges.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://www.nlrb.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public case database.",
    },
    // NPI - National Provider Identifier (Healthcare)
    SourceDescriptor {
        id: "npi",
        title: "NPI Registry",
        description: "CMS National Provider Identifier registry for healthcare providers.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://npiregistry.cms.hhs.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; bulk data download available.",
    },
    // NPI Extended
    SourceDescriptor {
        id: "npi_extended",
        title: "NPI Extended Data",
        description: "Extended NPI data including affiliations and secondary identifiers.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://npiregistry.cms.hhs.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API and supplemental data.",
    },
    // ────── T28: FARA & FINRA ───────────────────────────────────────────────

    // FARA - Foreign Agents Registration Act
    SourceDescriptor {
        id: "fara",
        title: "FARA Registrations",
        description: "U.S. Department of Justice Foreign Agents Registration Act filings.",
        category: SourceCategory::Government,
        homepage_url: "https://www.justice.gov/nsd/fara",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public database; searchable registrations.",
    },
    // FINRA BrokerCheck
    SourceDescriptor {
        id: "finra_brokercheck",
        title: "FINRA BrokerCheck",
        description: "FINRA broker-dealer, representative, and firm disciplinary history.",
        category: SourceCategory::Regulatory,
        homepage_url: "https://brokercheck.finra.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: true,
        access_instructions: "Public API; no authentication required.",
    },
    // ────── T29: International Registries & Sanctions ──────────────────────────

    // Australia DFAT Sanctions
    SourceDescriptor {
        id: "australia_dfat_sanctions",
        title: "Australia DFAT Sanctions",
        description: "Australian Department of Foreign Affairs and Trade sanctions designations.",
        category: SourceCategory::Sanctions,
        homepage_url: "https://www.dfat.gov.au/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public list; downloadable from DFAT website.",
    },
    // Canada Corporations
    SourceDescriptor {
        id: "canada_corporations",
        title: "Canada Corporations",
        description: "Canada Corporations database: corporate registrations and filings.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.ic.gc.ca/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; corporate name search available.",
    },
    // Canada SEMA Sanctions
    SourceDescriptor {
        id: "canada_sema_sanctions",
        title: "Canada SEMA Sanctions",
        description: "Special Economic Measures Act (SEMA) sanctions list.",
        category: SourceCategory::Sanctions,
        homepage_url: "https://www.international.gc.ca/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public XML list from Global Affairs Canada.",
    },
    // OpenSanctions - Aggregated sanctions
    SourceDescriptor {
        id: "opensanctions",
        title: "OpenSanctions",
        description: "OpenSanctions curated and enriched global sanctions lists.",
        category: SourceCategory::Sanctions,
        homepage_url: "https://www.opensanctions.org/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Community data; API available; optional API key for commercial use.",
    },
    // UK Companies House
    SourceDescriptor {
        id: "uk_companies_house",
        title: "UK Companies House",
        description: "UK Companies House corporate registrations, officers, and filings.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.companieshouse.gov.uk/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public data; API key optional for enhanced access.",
    },
    // UK Corporate Intelligence
    SourceDescriptor {
        id: "uk_corporate_intelligence",
        title: "UK Corporate Intelligence",
        description: "UK corporate ownership and structure intelligence.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.companieshouse.gov.uk/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public data; derived from Companies House.",
    },
    // UK HMT Sanctions
    SourceDescriptor {
        id: "uk_hmt_sanctions",
        title: "UK HMT Sanctions",
        description: "UK HM Treasury sanctions list (post-Brexit).",
        category: SourceCategory::Sanctions,
        homepage_url: "https://www.gov.uk/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public UK sanctions list.",
    },
    // ────── T30: Aviation & Maritime ─────────────────────────────────────────

    // FAA N-Number - Aircraft Registration
    SourceDescriptor {
        id: "faa_nnumber",
        title: "FAA N-Number Registry",
        description: "FAA aircraft registration database (N-Numbers).",
        category: SourceCategory::Government,
        homepage_url: "https://registry.faa.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public FAA registry; searchable interface.",
    },
    // Maritime AIS - Automatic Identification System
    SourceDescriptor {
        id: "maritime_ais",
        title: "Maritime AIS",
        description: "Vessel tracking via Automatic Identification System data.",
        category: SourceCategory::Government,
        homepage_url: "https://www.marinetraffic.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public AIS data; various web scrapers and APIs available.",
    },
    // ────── T31: Property & UCC Filings ──────────────────────────────────────

    // Assessor Portals - County property records
    SourceDescriptor {
        id: "assessor_portals",
        title: "Assessor Portals",
        description: "County assessor property valuations, tax records, and deed transfers.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.county.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "County-specific portals; public access varies.",
    },
    // Delaware Franchise Tax
    SourceDescriptor {
        id: "delaware_franchise_tax",
        title: "Delaware Franchise Tax",
        description: "Delaware Division of Corporations franchise tax records.",
        category: SourceCategory::Corporate,
        homepage_url: "https://delaware.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public Delaware corporate records.",
    },
    // Property Valuation - Zillow-like data
    SourceDescriptor {
        id: "property_valuation",
        title: "Property Valuation",
        description: "Property market valuations, price history, and assessments.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.zillow.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Various APIs; some require authentication.",
    },
    // SEC 13D / 13G - Beneficial ownership disclosure
    SourceDescriptor {
        id: "sec_13d_13g",
        title: "SEC 13D/13G",
        description: "SEC Schedule 13D/13G beneficial ownership disclosures.",
        category: SourceCategory::Government,
        homepage_url: "https://www.sec.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public SEC filings database.",
    },
    // UCC Filings - Uniform Commercial Code secured transactions
    SourceDescriptor {
        id: "ucc_filings",
        title: "UCC Filings",
        description: "Uniform Commercial Code secured transaction filings from state repositories.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.sos.state.us/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "State-specific UCC databases; mostly public with search fees.",
    },
    // ────── T32: Academic & Media Intelligence ───────────────────────────────

    // Bluesky
    SourceDescriptor {
        id: "bluesky",
        title: "Bluesky",
        description: "Bluesky social network profiles and posts.",
        category: SourceCategory::Media,
        homepage_url: "https://bsky.app/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; no authentication required for basic queries.",
    },
    // Common Crawl
    SourceDescriptor {
        id: "common_crawl",
        title: "Common Crawl",
        description: "Common Crawl web archive index and full-page captures.",
        category: SourceCategory::Media,
        homepage_url: "https://commoncrawl.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public data; BigQuery and S3 access available.",
    },
    // Hacker News
    SourceDescriptor {
        id: "hackernews",
        title: "Hacker News",
        description: "Hacker News user profiles and submission history.",
        category: SourceCategory::Media,
        homepage_url: "https://news.ycombinator.com/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; Firebase real-time endpoint.",
    },
    // LinkedIn Public
    SourceDescriptor {
        id: "linkedin_public",
        title: "LinkedIn Public Profiles",
        description: "LinkedIn public profile pages and professional history.",
        category: SourceCategory::Osint,
        homepage_url: "https://www.linkedin.com/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public profiles; web scraping with rate limiting.",
    },
    // Listen Notes - Podcasts
    SourceDescriptor {
        id: "listen_notes",
        title: "Listen Notes",
        description: "Listen Notes podcast search and episode data.",
        category: SourceCategory::Media,
        homepage_url: "https://www.listennotes.com/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "API key optional; commercial use requires paid tier.",
    },
    // Mastodon
    SourceDescriptor {
        id: "mastodon",
        title: "Mastodon",
        description: "Mastodon user profiles and statuses from any Mastodon instance.",
        category: SourceCategory::Media,
        homepage_url: "https://mastodon.social/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; no authentication required for public data.",
    },
    // ORCID - Open Researcher and Contributor ID
    SourceDescriptor {
        id: "orcid",
        title: "ORCID",
        description: "ORCID researcher profiles with employment, education, and publications.",
        category: SourceCategory::Academic,
        homepage_url: "https://orcid.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; bulk data available.",
    },
    // Reddit
    SourceDescriptor {
        id: "reddit",
        title: "Reddit",
        description: "Reddit user profiles and submission history.",
        category: SourceCategory::Media,
        homepage_url: "https://www.reddit.com/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public data; Pushshift archive and official API available.",
    },
    // SEC XBRL - eXtensible Business Reporting Language
    SourceDescriptor {
        id: "sec_xbrl",
        title: "SEC XBRL",
        description: "SEC structured financial data from XBRL submissions.",
        category: SourceCategory::Government,
        homepage_url: "https://www.sec.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public SEC data.gov API and bulk downloads.",
    },
    // Semantic Scholar
    SourceDescriptor {
        id: "semantic_scholar",
        title: "Semantic Scholar",
        description: "Semantic Scholar academic paper search powered by AI.",
        category: SourceCategory::Academic,
        homepage_url: "https://www.semanticscholar.org/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; free tier available.",
    },
    // YouTube
    SourceDescriptor {
        id: "youtube",
        title: "YouTube",
        description: "YouTube channel and video metadata via official API.",
        category: SourceCategory::Media,
        homepage_url: "https://www.youtube.com/",
        auth_requirement: AuthRequirement::Required,
        credential_field: Some("youtube_api_key"),
        enabled_by_default: false,
        access_instructions: "Requires YouTube API key and Google Cloud project setup.",
    },
    // ────── T33: Cryptocurrency & DeFi ──────────────────────────────────────

    // Blockchain Explorer
    SourceDescriptor {
        id: "blockchain_explorer",
        title: "Blockchain Explorer",
        description: "Blockchain transaction data from Etherscan and similar explorers.",
        category: SourceCategory::Crypto,
        homepage_url: "https://etherscan.io/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public API; API key optional for better rate limits.",
    },
    // DeFi Protocols
    SourceDescriptor {
        id: "defi_protocols",
        title: "DeFi Protocols",
        description: "Decentralized finance protocol data: Uniswap, Aave, Compound.",
        category: SourceCategory::Crypto,
        homepage_url: "https://www.uniswap.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public blockchain data; GraphQL endpoints available.",
    },
    // Exchange Transparency
    SourceDescriptor {
        id: "exchange_transparency",
        title: "Exchange Transparency",
        description: "Cryptocurrency exchange proof-of-reserves and AML compliance reports.",
        category: SourceCategory::Crypto,
        homepage_url: "https://www.coinbase.com/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public reports; varies by exchange.",
    },
    // Tornado Screening - Privacy pool tracking
    SourceDescriptor {
        id: "tornado_screening",
        title: "Tornado Cash Screening",
        description: "Tornado Cash mixer transaction screening and detection.",
        category: SourceCategory::Crypto,
        homepage_url: "https://tornado.cash/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public blockchain analysis; various tools available.",
    },
    // ────── T35: Environmental & Permits ────────────────────────────────────

    // Carbon Registries
    SourceDescriptor {
        id: "carbon_registries",
        title: "Carbon Registries",
        description: "Carbon credit and offset registries and verification.",
        category: SourceCategory::Environmental,
        homepage_url: "https://www.verra.org/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public registries; searchable databases.",
    },
    // EPA Superfund
    SourceDescriptor {
        id: "epa_superfund",
        title: "EPA Superfund",
        description: "EPA Superfund National Priorities List contaminated sites.",
        category: SourceCategory::Environmental,
        homepage_url: "https://www.epa.gov/superfund",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public EPA database; downloadable lists.",
    },
    // SEC Climate Disclosure
    SourceDescriptor {
        id: "sec_climate",
        title: "SEC Climate Disclosure",
        description: "SEC climate-related risk disclosures from 10-K/10-Q filings.",
        category: SourceCategory::Environmental,
        homepage_url: "https://www.sec.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public SEC filings; text search available.",
    },
    // State Environmental Permits
    SourceDescriptor {
        id: "state_env_permits",
        title: "State Environmental Permits",
        description: "State environmental agency permits and compliance records.",
        category: SourceCategory::Environmental,
        homepage_url: "https://www.epa.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "State-specific portals; public access varies.",
    },
    // ────── T36: Healthcare ──────────────────────────────────────────────────

    // FDA Warnings (already in T27)
    // (covered above)

    // ────── T37: Business & Legal Intelligence ───────────────────────────────

    // Amazon Authors
    SourceDescriptor {
        id: "amazon_authors",
        title: "Amazon Authors",
        description: "Amazon author profiles and book metadata.",
        category: SourceCategory::Media,
        homepage_url: "https://www.amazon.com/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public profiles; web scraping.",
    },
    // ────── T38: Nonprofit Intelligence ───────────────────────────────────────

    // GuideStar / Candid
    SourceDescriptor {
        id: "guidestar_candid",
        title: "Candid (GuideStar)",
        description: "GuideStar nonprofit data now under Candid foundation.",
        category: SourceCategory::Nonprofit,
        homepage_url: "https://www.candid.org/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Free basic access; premium API available.",
    },
    // IRS 1023 - Tax-exempt application
    SourceDescriptor {
        id: "irs_1023",
        title: "IRS Form 1023",
        description: "IRS Form 1023 tax-exempt organization applications.",
        category: SourceCategory::Nonprofit,
        homepage_url: "https://www.irs.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public PDF filings; archive available.",
    },
    // IRS 990 XML
    SourceDescriptor {
        id: "irs_990_xml",
        title: "IRS Form 990 XML",
        description: "IRS Form 990 structured XML from electronic filing system.",
        category: SourceCategory::Nonprofit,
        homepage_url: "https://www.irs.gov/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public data.gov API and bulk XML archives.",
    },
    // ────── T41: EU Business Registers ───────────────────────────────────────

    // EU BRIS
    SourceDescriptor {
        id: "eu_bris",
        title: "EU BRIS",
        description: "EU Business Registers Interconnection System company data.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.ibis-web.eu/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public EU interconnected system.",
    },
    // France Infogreffe
    SourceDescriptor {
        id: "france_infogreffe",
        title: "France Infogreffe",
        description: "French commercial register (Infogreffe) company data.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.infogreffe.fr/",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public search; API key optional for bulk access.",
    },
    // Germany Handelsregister
    SourceDescriptor {
        id: "germany_handelsregister",
        title: "Germany Handelsregister",
        description: "German Chamber of Commerce commercial register.",
        category: SourceCategory::Corporate,
        homepage_url: "https://www.handelsregisterbekanntmachungen.de/",
        auth_requirement: AuthRequirement::None,
        credential_field: None,
        enabled_by_default: false,
        access_instructions: "Public German commercial registry.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_by_id_returns_known_fetchers() {
        assert!(source_by_id("fec").is_some());
        assert!(source_by_id("opencorporates").is_some());
        assert!(source_by_id("ofac_sdn").is_some());
    }

    #[test]
    fn test_source_by_id_returns_none_for_unknown() {
        assert!(source_by_id("unknown_source").is_none());
    }

    #[test]
    fn test_all_source_ids_match_settings() {
        let catalog_ids: std::collections::BTreeSet<_> = all_source_ids().into_iter().collect();
        // Catalog should have all settings fetchers at minimum
        for fetcher_id in crate::domain::settings::KNOWN_FETCHERS {
            assert!(
                catalog_ids.contains(fetcher_id),
                "Fetcher {fetcher_id} in KNOWN_FETCHERS but not in catalog"
            );
        }
    }

    #[test]
    fn test_sources_by_category_returns_sorted() {
        let gov_sources = sources_by_category(SourceCategory::Government);
        assert!(!gov_sources.is_empty());
        // Verify they are sorted by title
        for window in gov_sources.windows(2) {
            if let (Some(a), Some(b)) = (window.first(), window.get(1)) {
                assert!(a.title <= b.title);
            }
        }
    }

    #[test]
    fn test_all_sources_enabled_only_filters() {
        let all = all_sources(false);
        let enabled = all_sources(true);
        assert!(all.len() >= enabled.len());
        for source in &enabled {
            assert!(source.enabled_by_default);
        }
    }

    #[test]
    fn test_descriptor_has_required_fields() {
        for source in SOURCES {
            assert!(!source.id.is_empty(), "Source id cannot be empty");
            assert!(!source.title.is_empty(), "Source title cannot be empty");
            assert!(
                !source.description.is_empty(),
                "Source description cannot be empty"
            );
            assert!(
                !source.homepage_url.is_empty(),
                "Source homepage_url cannot be empty"
            );
            assert!(
                !source.access_instructions.is_empty(),
                "Source access_instructions cannot be empty"
            );
        }
    }

    #[test]
    fn test_auth_requirement_implications() {
        // Sources with Required auth should have credential_field set
        for source in SOURCES {
            if source.auth_requirement == AuthRequirement::Required {
                assert!(
                    source.credential_field.is_some(),
                    "Source {} requires auth but has no credential_field",
                    source.id
                );
            }
        }
    }

    #[test]
    fn test_source_category_display_names() {
        let categories = [
            SourceCategory::Corporate,
            SourceCategory::Sanctions,
            SourceCategory::Courts,
            SourceCategory::Media,
            SourceCategory::Academic,
            SourceCategory::Crypto,
            SourceCategory::Nonprofit,
            SourceCategory::Regulatory,
            SourceCategory::Environmental,
            SourceCategory::Osint,
            SourceCategory::Government,
        ];
        for cat in categories {
            assert!(!cat.display_name().is_empty());
        }
    }

    #[test]
    fn test_auth_requirement_display_names() {
        assert!(!AuthRequirement::None.display_name().is_empty());
        assert!(!AuthRequirement::Optional.display_name().is_empty());
        assert!(!AuthRequirement::Required.display_name().is_empty());
    }
}
