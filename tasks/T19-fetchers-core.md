# T19 — 12 ported fetcher binaries (FEC, SEC, USASpending, lobbying, OFAC, ICIJ, 990, Census, EPA, FDIC, OSHA, SAM)

> **Depends on**: T-fetcher-framework, T-stygian-integration.

## Goal

Port all 12 fetch_*.py scripts from OpenPlanter to Rust fetcher binaries.
Each fetches a specific public data source, respects rate limits, and writes
NDJSON output. Use stygian-graph pipelines where multi-step or JS-rendered
extraction is needed.


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

- Each fetcher has a unit test that mocks the HTTP response and asserts correct NDJSON output format.
- Test: FEC fetcher constructs correct query params for candidate search.
- Test: SEC EDGAR ticker→CIK resolution parses company_tickers.json fixture.
- Test: Senate lobbying XML parser extracts registrant and client names from LD-2 fixture.
- Test: OFAC SDN XML parser extracts entity name and program from fixture.


### 2. GREEN — Implement to pass

- fetch-fec: GET https://api.open.fec.gov/v1/candidates, /committees, /schedules/schedule_a, /totals. API key passed as api_key param.
- fetch-sec-edgar: GET https://data.sec.gov/submissions/{CIK}.json. Accepts --ticker or --cik; resolve ticker→CIK via https://www.sec.gov/files/company_tickers.json.
- fetch-usaspending: POST https://api.usaspending.gov/api/v2/search/spending_by_award (paginated via page param).
- fetch-senate-lobbying: GET https://soprweb.senate.gov/index.aspx?event=processSearchCriteria (XML ZIP downloads; parse LD-1/LD-2/LD-203 XML).
- fetch-ofac-sdn: GET https://www.treasury.gov/ofac/downloads/sdn.xml; parse XML to NDJSON.
- fetch-icij-leaks: GET https://offshoreleaks.icij.org/api/... ; parse entity/officer/address nodes.
- fetch-propublica-990: GET https://projects.propublica.org/nonprofits/api/v2/search.json.
- fetch-census-acs: GET https://api.census.gov/data/{year}/acs/acs5 with variable list.
- fetch-epa-echo: GET https://echo.epa.gov/rest/services/... facility enforcement records.
- fetch-fdic: GET https://banks.data.fdic.gov/api/... financial institution data.
- fetch-osha: GET https://enforcedata.dol.gov/views/data_summary.php inspection records.
- fetch-sam-gov: GET https://api.sam.gov/entity-information/v3/entities with API key.
- Where applicable, route through stygian-graph pipelines (stygian feature): ICIJ uses browser for JS rendering; SAM.gov benefits from AI extraction node for unstructured fields.


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

Update PROGRESS.md row for T19 to `[x]`.
Commit: `feat(fetchers-core): implement 12 ported fetcher binaries (fec, sec, usaspending, lobbying, ofac, icij, 990, census, epa, fdic, osha, sam)`
