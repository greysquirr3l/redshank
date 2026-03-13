# T20 — 14 new fetcher binaries expanding corporate, sanctions, courts, and property intelligence

> **Depends on**: T-fetchers-core.

## Goal

Add 14 new data-fetcher binaries that significantly expand Redshank's
investigative reach: beneficial ownership (FinCEN BOI, OpenCorporates, GLEIF),
additional sanctions layers (UN, EU, World Bank), courts (CourtListener/RECAP),
House lobbying, federal audits (FAC), granular contracts (FPDS-NG),
entity disambiguation (Wikidata SPARQL), media intelligence (GDELT),
state corporate registries and county property records (both via stygian-graph
browser + AI extraction pipelines).


## Project Context

- Project: `redshank` — Redshank is an autonomous recursive language-model investigation agent written
in Rust 1.94 (edition 2024). It ingests heterogeneous public datasets — campaign
finance, lobbying disclosures, federal contracts, corporate registries,
sanctions lists (OFAC, UN, EU, World Bank), property records, nonprofit
filings, corporate registries (GLEIF, OpenCorporates, FinCEN BOI, state SOS
portals), federal courts (RECAP/CourtListener), individual-person OSINT
(breach exposure, username enumeration across 300+ platforms, voter rolls,
github profiles, WHOIS history, patent/trademark inventors), and media
intelligence (GDELT) — resolves entities across all of them, and surfaces
non-obvious connections through evidence-backed analysis written into a live
knowledge-graph wiki.

The agent runs a tool-calling loop that can recursively delegate subtasks to
child agent invocations, condense context on long runs, apply a cheap judge
model to evaluate acceptance criteria, and stream its reasoning to an interactive
ratatui TUI. Web fetches use stygian-graph pipelines (with optional stygian-browser
anti-detection automation for JS-rendered pages). A compiled binary ships as a
single executable with no Python or Node.js runtime dependency.

- Language: rust
- Architecture: hexagonal-ddd-cqrs-security-first



## Strategy: TDD (Red-Green-Refactor)

### 1. RED — Write failing tests first

- Each of the 14 new fetchers has a unit test mocking the HTTP/SPARQL response.
- Test: GLEIF fetcher extracts LEI, legal name, and parent LEI from fixture JSON.
- Test: OpenCorporates fetcher returns company number and registered address for a DE fixture.
- Test: UN sanctions XML parser extracts aliases and passport identifiers from fixture.
- Test: EU sanctions XML parser handles both individual and entity subject types.
- Test: CourtListener fetcher constructs correct query string and parses docket fixture.
- Test: Wikidata SPARQL fetcher serialises query, POSTs with correct Content-Type, parses bindings.
- Test: GDELT fetcher URL-encodes entity name and parses artlist JSON fixture.
- Test (stygian feature): state-sos pipeline config loads and validates without launching a browser.
- Test (stygian feature): county-property ACRIS endpoint returns owner name from mock JSON response.


### 2. GREEN — Implement to pass

- fetch-fincen-boi: POST https://boiefiling.fincen.gov/api/v1/... to search the FinCEN Beneficial Ownership database (post-CTA 2024 filings). API key required. Output: entity name, jurisdiction, beneficial owners (name, DOB hash, address). Critical for piercing shell company layers.
- fetch-gleif: GET https://api.gleif.org/api/v1/lei-records?filter[entity.legalName]={name} — GLEIF LEI Registry. Returns Legal Entity Identifier, jurisdiction, registration authority, parent LEI (ownership chain). No auth required. Use as the canonical cross-linking ID for any entity appearing in SEC/SAM/USASpending.
- fetch-opencorporates: GET https://api.opencorporates.com/v0.4/companies/search?q={name}&jurisdiction_code={code} — 200+ jurisdiction aggregator. API key for high-volume. Vital for DE/WY/NV shell companies and foreign subsidiaries the EDGAR graph misses.
- fetch-house-lobbying: GET https://clerkapi.house.gov/Lobbying/... LD-1/LD-2 XML downloads from House Clerk (separate system from Senate SOPR). Parse registrant, client, issue codes, and covered officials. Combine output with senate-lobbying for full federal lobbying picture.
- fetch-recap-courtlistener: GET https://www.courtlistener.com/api/rest/v4/dockets/?q={query} — CourtListener / RECAP free federal court archive. Returns case name, court, docket entries, parties, attorneys. No auth for basic use; API key for bulk. Use stygian-graph http→ai_extract pipeline to pull case summaries from docket PDFs via vision API.
- fetch-un-sanctions: GET https://scsanctions.un.org/resources/xml/en/consolidated.xml — UN Security Council Consolidated Sanctions List XML. Parse individual/entity records: aliases, addresses, identifiers (passport, national ID), listing reason. Complements OFAC for entities sanctioned by multilateral bodies but not the US.
- fetch-eu-sanctions: GET https://webgate.ec.europa.eu/fsd/fsf/public/files/xmlFullSanctionsList_1_1/content — EU CFSP/RELEX consolidated financial sanctions XML. Covers entities sanctioned under EU law that may not overlap with OFAC or UN lists. Parse subject, nameAlias, identificationDetail.
- fetch-world-bank-debarred: GET https://apigwext.worldbank.org/dvsvc/v1.0/json/APPLICATION/ADOBE_ACROBAT/FIRM/debarredFirms — World Bank debarred and cross-debarred firms JSON. Covers multilateral development bank sanctions. Cross-debarment flag indicates entity banned across MDB signatories.
- fetch-federal-audit: GET https://facdissem.census.gov/api/v1.0/submissions?... — Federal Audit Clearinghouse (FAC) single-audit database. Covers any organisation that spent $750k+ in federal grants. Returns findings, questioned costs, material weaknesses. JSON API no auth.
- fetch-fpds: POST https://api.sam.gov/prod/opportunities/v2/search — FPDS-NG contract awards at a more granular level than USASpending: individual line items, modification history, award type codes, NAICS. Use this in parallel with USASpending to cross-validate and surface discrepancies.
- fetch-wikidata: POST https://query.wikidata.org/sparql with SPARQL queries. Wikidata is the backbone for entity disambiguation: links Q-IDs across corporate registries, government roles, sanctions lists, and news mentions. Key queries: politician→company board memberships, officer→organisation affiliations, company→subsidiary tree. Use the sparql crate or raw reqwest POST with Accept: application/sparql-results+json.
- fetch-state-sos: stygian-graph pipeline with browser + AI extraction nodes. Target DE (https://icis.corp.delaware.gov), WY (https://wyobiz.wyo.gov), NV (https://esos.nv.gov), FL (https://search.sunbiz.org). These are JS-heavy portals with no public API; stygian-browser Advanced stealth + claude ai_extract node parses the entity detail page. Output: registered agent, officers, formation date, status. Pipeline config stored as TOML in src/pipelines/state_sos/.
- fetch-county-property: stygian-graph pipeline. Target county assessor portals for high-interest jurisdictions (NYC ACRIS https://a836-acris.nyc.gov, Miami-Dade, LA, Cook County). ACRIS has a JSON API (https://data.cityofnewyork.us/resource/636b-3b5g.json); others require browser + AI extraction. Output: owner name, LLC/trust chain, assessed value, deed history. Property→beneficial-owner chains are a key money-laundering vector.
- fetch-gdelt: GET https://api.gdeltproject.org/api/v2/doc/doc?query={entity}&mode=artlist&format=json — GDELT Global Event/Media database. Returns news article metadata (URL, theme codes, image, tone score) mentioning the queried entity. Use to surface when an entity under investigation appears in news coverage and what context (conflict, corruption, legal). Combine with fetch-recap results to correlate legal filings with media coverage timeline.


### 3. REFACTOR — Clean up while green

- Remove duplication
- Improve naming and structure
- Keep all tests passing


## Housekeeping: TODO / FIXME Sweep

Before running preflight, scan all files you created or modified in this task for
`TODO`, `FIXME`, `HACK`, `XXX`, and similar markers.

- **Resolve** any that fall within the scope of this task's goal.
- **Leave in place** any that reference work belonging to a later task or phase — but ensure they include a task reference (e.g. `// TODO(T07): wire up auth adapter`).
- **Remove** any placeholder markers that are no longer relevant after your implementation.

If none are found, move on.

## Preflight

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

## Exit Criteria

- [ ] All code compiles without errors or warnings
- [ ] All tests pass
- [ ] Linter passes with no warnings
- [ ] Implementation matches the goal described above
- [ ] No unresolved TODO/FIXME/HACK markers that belong to this task's scope

## After Completion

Update PROGRESS.md row for T20 to `[x]`.
Commit: `feat(fetchers-extended): implement 14 new fetcher binaries expanding corporate, sanctions, courts, and property intelligence`
