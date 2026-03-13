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
| T01 — Cargo workspace, CI, and repo hygiene | `[x]` | Complete |

---


## Phase 2 — Domain Model

> Depends on: Phase 1 all complete


| Task | Status | Notes |
|---|---|---|
| T02 — Domain model: entities, value objects, aggregates, domain events, and CQRS command/query types | `[x]` | Complete |
| T03 — Credential bundle, storage (chmod 600), and resolution order | `[x]` | Complete |
| T04 — Persistent settings (per-provider default model + reasoning effort) | `[x]` | Complete |
| T05 — Security domain model: AuthContext, Permission, SecurityPolicy (fail-secure, DDD-Lite) | `[x]` | Complete |

---


## Phase 3 — LLM Provider Layer

> Depends on: Phase 2 all complete


| Task | Status | Notes |
|---|---|---|
| T06 — AnthropicModel: native Messages API with SSE streaming and thinking budgets | `[x]` | Complete |
| T07 — OpenAICompatibleModel: OpenAI, OpenRouter, Cerebras, Ollama | `[x]` | Complete |
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

- T01: ReplayLog trait uses RPITIT (`impl Future`) which is not dyn-compatible. Use generics (`T: ReplayLog`) not `dyn ReplayLog`. Changed `child()` to `child_path()` returning a String.
- T01: redshank-core with `--no-default-features` has zero I/O deps — domain purity verified via `cargo tree`.
- T01: edition 2024 compiles fine on stable 1.94. RPITIT works natively (no async-trait crate needed).
- T02: AgentSession is the aggregate root — SessionStore port should reference AgentSession, not a flat Session struct.
- T02: ProviderKind::from_model_name must check specific prefixes (ollama/, cerebras/) before the generic contains('/') fallback for OpenRouter.
- T02: Use `#[default]` derive attribute on enum variants instead of manual Default impls — clippy enforces `derivable_impls`.
- T02: CredentialGuard<T> with Deref + "***REDACTED***" Debug/Display is the zero-cost secret masking pattern. serde(transparent) keeps JSON clean.
- T03: Credential adapter uses only std I/O (no tokio/reqwest needed). set_owner_only_perms uses cfg(unix) platform gate.
- T03: Resolution order: explicit > env vars > .env file > workspace store > user store. merge_missing() fills only None fields.
- T04: PersistentSettings uses skip_serializing_if = "Option::is_none" for clean JSON. Unknown keys silently ignored (serde default).
- T05: SecurityPolicy trait is object-safe (`&dyn SecurityPolicy`). StaticPolicy uses explicit match arms per role — no role ordering/comparison.
- T05: UserId newtype wraps Uuid, no Copy — forces intentional passing. UserId::system() returns nil UUID.
- T05: AuthContext carries CredentialGuard<String> for session_token — redacted in Debug output. All downstream CQRS types needed zero changes (AuthContext used opaquely).
- T06: SSE parsing uses byte-level parse_sse_events() → StreamAccumulator pattern. Tool-call JSON fragments accumulate via InProgressToolCall vec, then joined and parsed.
- T06: Thinking budgets only for models matching `contains("opus-4")` — use let-chain to collapse nested ifs. All Claude models currently share 200k context window.
- T06: CustomDebug on AnthropicModel excludes api_key field — CredentialGuard alone isn't enough since the struct holds it as a field.
- T07: Single OpenAICompatibleModel serves OpenAI/OpenRouter/Cerebras/Ollama via for_provider() factory. OpenRouter needs HTTP-Referer + X-Title headers. Ollama gets 120s timeout.
- T07: OpenAI SSE uses `data: [DONE]` terminator — must filter before JSON parse. Tool call deltas indexed by `index` field, not by content_block events like Anthropic.
