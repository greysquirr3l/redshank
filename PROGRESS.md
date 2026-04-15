# redshank — Implementation Progress

> Orchestrator reads this file at the start of each loop iteration.
> Subagents update this file after completing a task.

## Status Legend

- `[ ]` — Not started
- `[~]` — In progress (claimed by a subagent)
- `[x]` — Completed
- `[!]` — Blocked / needs human input

---

## Phase 1 — Workspace Scaffold

| Task | Status | Notes |
| --- | --- | --- |
| T01 — Cargo workspace, CI, and repo hygiene | `[x]` | |

---

## Phase 2 — Domain Model

> Depends on: Phase 1 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T02 — Domain model: entities, value objects, aggregates, domain events, and CQRS command/query types | `[x]` | |
| T03 — Credential bundle, storage (chmod 600), and resolution order | `[x]` | |
| T04 — Persistent settings (per-provider default model + reasoning effort) | `[x]` | |
| T05 — Security domain model: AuthContext, Permission, SecurityPolicy (fail-secure, DDD-Lite) | `[x]` | |

---

## Phase 3 — LLM Provider Layer

> Depends on: Phase 2 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T06 — AnthropicModel: native Messages API with SSE streaming and thinking budgets | `[x]` | |
| T07 — OpenAICompatibleModel: OpenAI, OpenRouter, Cerebras, Ollama | `[x]` | |
| T08 — Provider builder and model-name inference | `[x]` | |

---

## Phase 4 — Tool Layer

> Depends on: Phase 3 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T09 — Tool definitions: JSON schemas and to_provider() converters | `[x]` | |
| T10 — WorkspaceTools: filesystem, shell, web, and parallel-write safety (adapters/tools/) | `[x]` | |
| T11 — Codex-style patch format parser and applier | `[x]` | |
| T12 — stygian-graph + stygian-browser integration for web fetching | `[x]` | |

---

## Phase 5 — Agent Engine

> Depends on: Phase 4 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T13 — ReplayLogger: JSONL delta-encoded LLM call log | `[x]` | |
| T14 — Context condensation and turn summaries | `[x]` | |
| T15 — RLMEngine: recursive tool-calling agent loop (application/services/ + CQRS command handler) | `[x]` | |

---

## Phase 6 — Wiki Graph

> Depends on: Phase 5 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T16 — WikiGraphModel: index parsing, cross-ref extraction, petgraph DAG | `[x]` | |

---

## Phase 7 — Session Persistence

> Depends on: Phase 6 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T17 — SQLite-backed session store (rusqlite) | `[x]` | |

---

## Phase 8 — Data Fetchers

> Depends on: Phase 7 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T18 — Fetcher trait, CLI entry points, and output conventions | `[x]` | |
| T19 — 12 ported fetcher binaries (FEC, SEC, USASpending, lobbying, OFAC, ICIJ, 990, Census, EPA, FDIC, OSHA, SAM) | `[x]` | |
| T20 — 14 new fetcher binaries expanding corporate, sanctions, courts, and property intelligence | `[x]` | |
| T21 — 8 individual-person OSINT fetchers (HIBP, GitHub, Wayback, WHOIS/RDAP, voter rolls, USPTO, username enum, social profiles) | `[x]` | |

---

## Phase 9 — TUI

> Depends on: Phase 8 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T22 — ratatui TUI: chat pane, wiki-graph canvas, activity indicator, REPL | `[x]` | |

---

## Phase 10 — CLI Entry Point

> Depends on: Phase 9 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T23 — clap CLI: run, tui, fetch, session, configure, version | `[x]` | |

---

## Phase 11 — Integration and Polish

> Depends on: Phase 10 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T24 — Full-stack integration tests with scripted model | `[x]` | |
| T25 — Coraline MCP tool bindings for self-directed code navigation | `[x]` | |
| T26 — README.md and AGENTS.md | `[x]` | |

---

## Phase 12 — Extended Fetchers

> Depends on: Phase 8 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T27 — 9 regulatory enforcement fetchers (CFPB, FTC, FDA, MSHA, NLRB, NHTSA, NPI, CFTC, GSA eOffer) | `[x]` | |
| T28 — 2 high-priority fetchers: DOJ FARA and FINRA BrokerCheck | `[x]` | |
| T29 — 6 international corporate/sanctions fetchers (UK, Canada, OpenSanctions, HMT, DFAT, SEMA) | `[x]` | |
| T30 — 2 asset intelligence fetchers: FAA N-Number and Maritime AIS | `[x]` | |
| T31 — 4 UCC and property intelligence fetchers | `[x]` | |
| T32 — 9 academic and media intelligence fetchers | `[x]` | |
| T33 — SEC XBRL structured financial data fetcher | `[x]` | |
| T34 — ICIJ Offshore Leaks Database API extension | `[x]` | |
| T35 — 4 extended social/professional profile fetchers | `[x]` | |
| T36 — 3 healthcare and pharmaceutical intelligence fetchers | `[x]` | |
| T37 — 3 business/legal database fetchers (BLS QCEW, PACER, SEC 13D/13G) | `[x]` | |
| T38 — 3 nonprofit and IRS intelligence fetchers (990 XML, 1023/1024A, GuideStar) | `[x]` | |
| T39 — 4 crypto and alternative finance fetchers | `[x]` | |
| T40 — 4 environmental and permits intelligence fetchers | `[x]` | |
| T41 — 3 EU business register fetchers (BRIS, Germany, France) | `[x]` | |

---

## Phase 13 — Configuration UX and Catalog

> Depends on: Phase 10 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T42 — source catalog metadata: categories, access requirements, and help text | `[x]` | Shared metadata registry for fetcher descriptions, categories, URLs, and access guidance. |
| T43 — provider endpoint config: hosted keys, local LLMs, and OpenAI-compatible URLs | `[x]` | First-class provider endpoint model for hosted vendors, Ollama, and generic OpenAI-compatible backends. |
| T44 — configuration queries and commands: merge catalog, settings, and credentials for UI use | `[x]` | CQRS layer for UI-ready configuration views and safe non-secret config updates. Query handlers return `ConfiguredSourceView` and `ConfiguredProviderView`. Command handlers stub I/O with TODO(T44). All 539 tests passing. |
| T45 — TUI configuration workbench: providers, data sources, and guided setup | `[x]` | Dedicated TUI settings surface for provider and scraper configuration. |
| T46 — configuration handler implementation: real persistence behind provider and source query/command handlers | `[x]` | Replaced TODO stubs in `GetConfiguredProviders`, `GetConfiguredSources`, `UpdateProviderConfiguration`, `UpdateSourceConfiguration` with real `SettingsStore` + `FileCredentialStore` I/O via a new `WorkspaceConfig` port and `WorkspaceConfigStore` adapter. |

---

## Phase 14 — Stygian Fallback Hardening

> Depends on: Phase 12 all complete, T12-stygian-integration, T26-readme-and-agents-md.

| Task | Status | Notes |
| --- | --- | --- |
| T47 — stygian capability detection and fallback policy | `[x]` | Added compile-time + runtime stygian-mcp availability probe and shared execution mode policy with fail-soft behavior for JS-heavy sources when fallback is unavailable. |
| T48 — wire JS-heavy fetchers to optional stygian fallback | `[x]` | Added `execution_mode_for_state_sos`, `execution_mode_for_county`, and `execution_mode_for_profile` in respective fetcher modules, all delegating to T47 policy layer. Added `FetcherHealth` enum + `▲`/`▼`/`?` glyph to TUI domain; footer renders colored stygian health indicator; engine pushes `AppEvent::FetcherHealthChanged`. |
| T49 — document stygian-mcp fallback operations and licensing boundaries | `[x]` | Added `docs/src/architecture/stygian-fallback.md` covering decision flow, feature flag, runtime probe config, TUI health indicator, setup (local + production), troubleshooting matrix, and licensing-boundary rationale. Updated `docs/src/SUMMARY.md`, `docs/src/getting-started/configuration.md`, and `README.md` with stygian setup section and probe config reference. |

---

## Accumulated Learnings

> Subagents append discoveries here after each task.
> The orchestrator reads this section at the start of every iteration
> to avoid repeating past mistakes.

- T27: When testing version strings, use semver prefix matching (e.g., `starts_with("0.1.")`) rather than exact version matching to avoid test failures when version is bumped.
- T27: Federal API patterns are consistent: extract_results/extract_hits helpers for parsing nested response structures, with optional field accessors chaining `.or_else()` for alternate field names across different API versions.
- T38: When credential growth pushes clippy over `too_many_lines`, factor env resolution through a small helper instead of duplicating bundle construction paths.
- T39: Normalize sanctioned-address constants to the same case as runtime address normalization or screening tests will fail for the wrong reason.
- T40: Environmental sources fit the existing parser-first fetcher pattern well; keep extraction local and lightweight before worrying about live portal complexity.
- T41: EU register coverage is easiest to keep green by separating BRIS, Germany, and France parsing into small fixtures with the Bodacc event parser living alongside the France company parser.
- T42: Building the source catalog with 107 fetchers as static const data enforces completeness; verify with test `all_source_ids_match_settings()` that catalog covers all KNOWN_FETCHERS. Auth requirements must have credential_field set to avoid test failures.
- T43: Treat provider routing as non-secret settings and keep credential lookup separate; a settings-aware builder plus tiny provider base-url overrides is enough to add local OpenAI-compatible endpoints without breaking hosted defaults.
- T44: Consolidate provider enum variants early (e.g., `ProviderKind::Ollama` → `ProviderKind::OpenAiCompatible`) to avoid cascading find-and-replace across 7+ files. Use `is_some_and()` instead of `map_or(false, ...)` to satisfy clippy. View models must expose only `bool has_credential`, never secret values. Stub handlers with TODO(T44) in TDD phase to allow implementation iteration before moving to T45.
- T45: Use an `ActiveScreen` enum discriminant in `handle_key_with_command` to dispatch per-screen; keep `KeyCode` imports local to each handler function (not module-level) so test modules need an explicit import. Provider static metadata (`ProviderKind` display info) belongs in the renderer — use const slices to drive both list and detail without heap allocations. Always add match arms for new `UiCommand` variants in the CLI entry point or you'll get a non-exhaustive-patterns error.
- T46: `PersistentSettings` field for provider config is `providers`, not `provider_endpoints`. `Role` has four variants: `Owner`, `Operator`, `Reader`, `Service` — `Service` lacks `ReadConfiguration`/`ConfigureProviders`/`ConfigureSources` and is the right choice for access-denied tests. `AuthContext` constructors are `system()` (Service role) and `owner(user_id, token)` — there is no `new()`. Always read the domain type's field names from source before writing test fixtures.
- T47: For optional integrations, separate compile-time gates from runtime health probes and test them independently by injecting a compile-gate flag in internal probe helpers; this keeps behavior deterministic on end-user machines.
- T48: Thin wiring functions (`execution_mode_for_*`) in each fetcher module keep the policy decision in one place (T47 `select_execution_mode`) and the routing call at the source layer. TUI health indicators driven by `AppEvent` keep rendering decoupled from the engine probe timing; `FetcherHealth::glyph()` returning a `&'static str` is cleaner than a `char` because ratatui `Span` takes `Into<String>`.
- T49: Clippy `missing_const_for_fn` propagates to callers of a non-const function — making `select_execution_mode` `const fn` required all three `execution_mode_for_*` wrappers to become `const fn` as well. Docs belong near the code they describe: a dedicated architecture page for a cross-cutting concern (stygian fallback) is more maintainable than inlining it in configuration or README.
