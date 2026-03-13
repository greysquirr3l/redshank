# T12 — stygian-graph + stygian-browser integration for web fetching

> **Depends on**: T-workspace-tools.

## Goal

Behind the 'stygian' feature flag, integrate stygian-graph pipelines as an
enhanced fetch_url implementation, and stygian-browser for JS-rendered pages.


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

- Test (stygian feature): run_scrape_pipeline with a minimal Node config returns Ok.
- Test: when stygian feature is absent, fetch_url compiles and works with reqwest fallback.
- Test: BrowserPool not instantiated when stygian feature is disabled.


### 2. GREEN — Implement to pass

- Add feature flags: default = [], stygian = ['dep:stygian-graph', 'dep:stygian-browser'].
- When stygian feature is enabled, fetch_url() checks if the URL is likely JS-rendered (heuristic: known SPAs, configurable domain list) and routes through stygian-browser BrowserPool.
- For bulk data-pipeline fetching (used by the fetcher scripts in phase 8), expose a run_scrape_pipeline(pipeline_json: &str) tool that executes a stygian-graph Node/Edge DAG.
- BrowserPool initialised lazily (once, shared across all fetch_url calls via Arc<OnceCell<BrowserPool>>).
- Stealth level: Advanced by default; configurable via AgentConfig.
- Non-stygian fallback: plain reqwest GET (always available without the feature flag).
- Anti-scraping note: use stygian only for public data — never to bypass paywalls or authenticated systems.


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

Update PROGRESS.md row for T12 to `[x]`.
Commit: `feat(stygian-integration): implement stygian-graph + stygian-browser integration for web fetching`
