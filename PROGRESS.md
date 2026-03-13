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
|---|---|---|
| T01 — Cargo workspace, CI, and repo hygiene | `[ ]` | |

---


## Phase 2 — Domain Model

> Depends on: Phase 1 all complete


| Task | Status | Notes |
|---|---|---|
| T02 — Domain model: entities, value objects, aggregates, domain events, and CQRS command/query types | `[ ]` | |
| T03 — Credential bundle, storage (chmod 600), and resolution order | `[ ]` | |
| T04 — Persistent settings (per-provider default model + reasoning effort) | `[ ]` | |
| T05 — Security domain model: AuthContext, Permission, SecurityPolicy (fail-secure, DDD-Lite) | `[ ]` | |

---


## Phase 3 — LLM Provider Layer

> Depends on: Phase 2 all complete


| Task | Status | Notes |
|---|---|---|
| T06 — AnthropicModel: native Messages API with SSE streaming and thinking budgets | `[ ]` | |
| T07 — OpenAICompatibleModel: OpenAI, OpenRouter, Cerebras, Ollama | `[ ]` | |
| T08 — Provider builder and model-name inference | `[ ]` | |

---


## Phase 4 — Tool Layer

> Depends on: Phase 3 all complete


| Task | Status | Notes |
|---|---|---|
| T09 — Tool definitions: JSON schemas and to_provider() converters | `[ ]` | |
| T10 — WorkspaceTools: filesystem, shell, web, and parallel-write safety (adapters/tools/) | `[ ]` | |
| T11 — Codex-style patch format parser and applier | `[ ]` | |
| T12 — stygian-graph + stygian-browser integration for web fetching | `[ ]` | |

---


## Phase 5 — Agent Engine

> Depends on: Phase 4 all complete


| Task | Status | Notes |
|---|---|---|
| T13 — ReplayLogger: JSONL delta-encoded LLM call log | `[ ]` | |
| T14 — Context condensation and turn summaries | `[ ]` | |
| T15 — RLMEngine: recursive tool-calling agent loop (application/services/ + CQRS command handler) | `[ ]` | |

---


## Phase 6 — Wiki Graph

> Depends on: Phase 5 all complete


| Task | Status | Notes |
|---|---|---|
| T16 — WikiGraphModel: index parsing, cross-ref extraction, petgraph DAG | `[ ]` | |

---


## Phase 7 — Session Persistence

> Depends on: Phase 6 all complete


| Task | Status | Notes |
|---|---|---|
| T17 — SQLite-backed session store (rusqlite) | `[ ]` | |

---


## Phase 8 — Data Fetchers

> Depends on: Phase 7 all complete


| Task | Status | Notes |
|---|---|---|
| T18 — Fetcher trait, CLI entry points, and output conventions | `[ ]` | |
| T19 — 12 ported fetcher binaries (FEC, SEC, USASpending, lobbying, OFAC, ICIJ, 990, Census, EPA, FDIC, OSHA, SAM) | `[ ]` | |
| T20 — 14 new fetcher binaries expanding corporate, sanctions, courts, and property intelligence | `[ ]` | |
| T21 — 8 individual-person OSINT fetchers (HIBP, GitHub, Wayback, WHOIS/RDAP, voter rolls, USPTO, username enum, social profiles) | `[ ]` | |

---


## Phase 9 — TUI

> Depends on: Phase 8 all complete


| Task | Status | Notes |
|---|---|---|
| T22 — ratatui TUI: chat pane, wiki-graph canvas, activity indicator, REPL | `[ ]` | |

---


## Phase 10 — CLI Entry Point

> Depends on: Phase 9 all complete


| Task | Status | Notes |
|---|---|---|
| T23 — clap CLI: run, tui, fetch, session, configure, version | `[ ]` | |

---


## Phase 11 — Integration and Polish

> Depends on: Phase 10 all complete


| Task | Status | Notes |
|---|---|---|
| T24 — Full-stack integration tests with scripted model | `[ ]` | |
| T25 — Coraline MCP tool bindings for self-directed code navigation | `[ ]` | |
| T26 — README.md and AGENTS.md | `[ ]` | |

---


## Accumulated Learnings

> Subagents append discoveries here after each task.
> The orchestrator reads this section at the start of every iteration
> to avoid repeating past mistakes.

_No learnings yet._
