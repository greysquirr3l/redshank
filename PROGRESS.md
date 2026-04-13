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
| T45 — TUI configuration workbench: providers, data sources, and guided setup | `[ ]` | Dedicated TUI settings surface for provider and scraper configuration. |

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
