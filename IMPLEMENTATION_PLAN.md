# redshank — Implementation Plan

## Overview

Redshank is an autonomous recursive language-model investigation agent written
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

**Architecture**: hexagonal-ddd-cqrs-security-first
**Language**: rust

---

## Phases

### Phase 1 — Workspace Scaffold

1. **T01 — Cargo workspace, CI, and repo hygiene**
   Bootstrap a compilable Cargo workspace with stub crates, GitHub Actions CI,
clippy config, deny.toml, and edition 2024 throughout.

### Phase 2 — Domain Model

1. **T02 — Domain model: entities, value objects, aggregates, domain events, and CQRS command/query types**
   Implement the pure domain layer in redshank-core/src/domain/ following DDD-Lite principles:
aggregate roots (AgentSession), value objects (AgentConfig, CredentialBundle), domain events
(DomainEvent enum), and the type vocabulary shared by every crate. Also stub the CQRS command
and query structs in application/commands/ and application/queries/.
Port trait interfaces (ModelProvider, SessionStore, etc.) live in src/ports/ so the domain
has zero knowledge of how they are implemented — see the separate hints below.

   _Depends on: workspace-scaffold_
2. **T03 — Credential bundle, storage (chmod 600), and resolution order**
   Implement the CredentialBundle (7 keys), credential stores (workspace + user-level),
.env file parser, and multi-source merge — mirroring agent/credentials.py.

   _Depends on: domain-types_
3. **T04 — Persistent settings (per-provider default model + reasoning effort)**
   Implement PersistentSettings stored in .redshank/settings.json — per-provider
default model name and global default reasoning effort.

   _Depends on: domain-types_
4. **T05 — Security domain model: AuthContext, Permission, SecurityPolicy (fail-secure, DDD-Lite)**
   Implement the security model in redshank-core/src/domain/auth.rs following the
security-first repository design principle: security rules are pure domain functions
with zero I/O — it is structurally impossible to call a repository method without
providing an AuthContext and having the policy evaluated. Default deny everywhere.
Based on docs/dev/security_first_repository_design.md.

   _Depends on: domain-types_

### Phase 3 — LLM Provider Layer

1. **T06 — AnthropicModel: native Messages API with SSE streaming and thinking budgets**
   Implement the ModelProvider port for Anthropic. Handle Claude models including
adaptive thinking (Opus 4.6+) and manual thinking budgets. Parse SSE events for
streaming content and tool-call delta accumulation.

   _Depends on: domain-types_
2. **T07 — OpenAICompatibleModel: OpenAI, OpenRouter, Cerebras, Ollama**
   Implement the ModelProvider port for all OpenAI-API-shaped providers.
Handle SSE streaming, tool-call delta accumulation, and per-provider
auth headers and base URLs.

   _Depends on: domain-types_
3. **T08 — Provider builder and model-name inference**
   Implement build_provider() factory: infers ProviderKind from model name,
constructs the right ModelProvider impl, and wraps it in Arc<dyn ModelProvider>.
Also implement list_models() for each provider.

   _Depends on: provider-anthropic, provider-openai-compat_

### Phase 4 — Tool Layer

1. **T09 — Tool definitions: JSON schemas and to_provider() converters**
   Define TOOL_DEFINITIONS as a static list of ToolDefinition structs covering all
19 tools. Implement to_anthropic_tools() and to_openai_tools() converters.

   _Depends on: domain-types_
2. **T10 — WorkspaceTools: filesystem, shell, web, and parallel-write safety (adapters/tools/)**
   Implement all 19 tools split across redshank-core/src/adapters/tools/ modules,
mirroring agent/tools.py: filesystem.rs (list_files, read_file, write_file, edit_file,
hashline_edit, read_image), shell.rs (run_shell, run_shell_bg, check/kill/cleanup_bg_jobs),
web.rs (web_search, fetch_url), patching.rs (apply_patch), stygian.rs (run_scrape_pipeline).
WorkspaceTools implements the ToolDispatcher port from src/ports/tool_dispatcher.rs.
All tool dispatch requires &AuthContext — operators and above may dispatch tools;
readers may not call write or shell tools.

   _Depends on: tool-defs, credentials, security-model_
3. **T11 — Codex-style patch format parser and applier**
   Port agent/patching.py to Rust: parse _**Begin Patch /**_ End Patch blocks
with Add File, Delete File, and Update File hunks. Two-pass matching: exact
then whitespace-normalised. Return ApplyReport.

   _Depends on: workspace-tools_
4. **T12 — stygian-graph + stygian-browser integration for web fetching**
   Behind the 'stygian' feature flag, integrate stygian-graph pipelines as an
enhanced fetch_url implementation, and stygian-browser for JS-rendered pages.

   _Depends on: workspace-tools_

### Phase 5 — Agent Engine

1. **T13 — ReplayLogger: JSONL delta-encoded LLM call log**
   Port agent/replay_log.py to Rust: append JSONL records for every LLM call
using delta encoding (seq 0 = full snapshot; seq N = delta since seq N-1).
Child loggers for subtasks use hierarchical IDs (root/d2s5).

   _Depends on: domain-types_
2. **T14 — Context condensation and turn summaries**
   Implement context-window tracking and turn-summary injection:
when token usage exceeds 75% of the model's context window, inject
a condensation turn that summarises past reasoning.

   _Depends on: provider-builder, domain-types_
3. **T15 — RLMEngine: recursive tool-calling agent loop (application/services/ + CQRS command handler)**
   Implement the core agent loop in redshank-core/src/application/services/engine.rs,
mirroring agent/engine.py. The CQRS entry point is RunInvestigationHandler in
application/commands/run_investigation.rs: it validates the command, checks idempotency
(mark as in-flight, return cached result on duplicate), enforces AuthContext via
can_run_agent(), then delegates to RLMEngine. Features: step budget, depth control,
subtask recursion (child RunInvestigationCommands), runtime policy enforcement,
acceptance-criteria judge, cancel() via CancellationToken.

   _Depends on: workspace-tools, provider-builder, replay-logger, context-condensation, patching, security-model_

### Phase 6 — Wiki Graph

1. **T16 — WikiGraphModel: index parsing, cross-ref extraction, petgraph DAG**
   Port agent/wiki_graph.py to Rust using petgraph. Parse wiki/index.md,
read individual entry files to extract bold cross-references, fuzzy-match
entity names across the registry, and build a petgraph::DiGraph.

   _Depends on: domain-types_

### Phase 7 — Session Persistence

1. **T17 — SQLite-backed session store (rusqlite)**
   Implement the SessionStore port using rusqlite. Replace OpenPlanter's scattered
JSONL + JSON files with a single .redshank/sessions.db SQLite database.
Schema: sessions, turns, events, artifacts, observations.

   _Depends on: domain-types, wiki-graph_

### Phase 8 — Data Fetchers

1. **T18 — Fetcher trait, CLI entry points, and output conventions**
   Define the DataFetcher trait and shared output conventions used by all 12
public-data fetcher scripts. Implement the shared HTTP client and rate-limit
helper.

   _Depends on: workspace-scaffold, credentials_
2. **T19 — 12 ported fetcher binaries (FEC, SEC, USASpending, lobbying, OFAC, ICIJ, 990, Census, EPA, FDIC, OSHA, SAM)**
   Port all 12 fetch_*.py scripts from OpenPlanter to Rust fetcher binaries.
Each fetches a specific public data source, respects rate limits, and writes
NDJSON output. Use stygian-graph pipelines where multi-step or JS-rendered
extraction is needed.

   _Depends on: fetcher-framework, stygian-integration_
3. **T20 — 14 new fetcher binaries expanding corporate, sanctions, courts, and property intelligence**
   Add 14 new data-fetcher binaries that significantly expand Redshank's
investigative reach: beneficial ownership (FinCEN BOI, OpenCorporates, GLEIF),
additional sanctions layers (UN, EU, World Bank), courts (CourtListener/RECAP),
House lobbying, federal audits (FAC), granular contracts (FPDS-NG),
entity disambiguation (Wikidata SPARQL), media intelligence (GDELT),
state corporate registries and county property records (both via stygian-graph
browser + AI extraction pipelines).

   _Depends on: fetchers-core_
4. **T21 — 8 individual-person OSINT fetchers (HIBP, GitHub, Wayback, WHOIS/RDAP, voter rolls, USPTO, username enum, social profiles)**
   Add 8 individual-person OSINT fetchers that fill a gap entirely absent from
OpenPlanter. These cover breach-exposure checking, public username/identity
correlation, historical web presence, domain registration history, voter
registration records, patent/trademark inventor data, and JS-rendered social
profile scraping. All sources are fully public or breach-notification services
that return only exposure metadata — never raw credential material.

   _Depends on: fetchers-extended, stygian-integration_

### Phase 9 — TUI

1. **T22 — ratatui TUI: chat pane, wiki-graph canvas, activity indicator, REPL**
   Implement the interactive TUI in redshank-tui using ratatui + crossterm.
Three-pane layout: sidebar (sessions + model), chat log, wiki-graph canvas.
Activity indicator, slash-command REPL, streaming content rendering.

   _Depends on: agent-engine, wiki-graph, session-store_

### Phase 10 — CLI Entry Point

1. **T23 — clap CLI: run, tui, fetch, session, configure, version**
   Wire the binary entry point using clap derive. Subcommands: run (headless),
tui (interactive TUI), fetch <source>, session list/delete/resume, configure
(interactive credential setup), version.

   _Depends on: tui-ratatui, session-store, fetchers-extended, credentials, settings_

### Phase 11 — Integration and Polish

1. **T24 — Full-stack integration tests with scripted model**
   Write end-to-end integration tests that exercise the full stack
(CLI → SessionRuntime → RLMEngine → WorkspaceTools) using a ScriptedModel
fixture — no live API calls.

   _Depends on: cli-entrypoint_
2. **T25 — Coraline MCP tool bindings for self-directed code navigation**
   When the agent is directed at the redshank workspace itself, it should be able
to use Coraline MCP tools (coraline_read_file, coraline_search, coraline_repo_map,
coraline_edit_file) via the tool layer. Wire these as additional ToolDefinitions
behind a 'coraline' feature flag.

   _Depends on: tool-defs, workspace-tools_
3. **T26 — README.md and AGENTS.md**
   Write README.md (project description, quickstart, feature list, fetcher list,
stygian integration notes, comparison with OpenPlanter) and AGENTS.md
(subagent instructions that mirror the orchestrator rules in plan.toml).

   _Depends on: cli-entrypoint_

---

## Preflight Commands

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

---

_Generated by [wiggum](https://github.com/greysquirr3l/wiggum)._
