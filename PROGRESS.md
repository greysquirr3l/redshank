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
| T01 — Cargo workspace, CI, and repo hygiene | `[x]` | Complete |

---

## Phase 2 — Domain Model

> Depends on: Phase 1 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T02 — Domain model: entities, value objects, aggregates, domain events, and CQRS command/query types | `[x]` | Complete |
| T03 — Credential bundle, storage (chmod 600), and resolution order | `[x]` | Complete |
| T04 — Persistent settings (per-provider default model + reasoning effort) | `[x]` | Complete |
| T05 — Security domain model: AuthContext, Permission, SecurityPolicy (fail-secure, DDD-Lite) | `[x]` | Complete |

---

## Phase 3 — LLM Provider Layer

> Depends on: Phase 2 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T06 — AnthropicModel: native Messages API with SSE streaming and thinking budgets | `[x]` | Complete |
| T07 — OpenAICompatibleModel: OpenAI, OpenRouter, Cerebras, Ollama | `[x]` | Complete |
| T08 — Provider builder and model-name inference | `[x]` | Complete |

---

## Phase 4 — Tool Layer

> Depends on: Phase 3 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T09 — Tool definitions: JSON schemas and to_provider() converters | `[x]` | 14 tests |
| T10 — WorkspaceTools: filesystem, shell, web, and parallel-write safety (adapters/tools/) | `[x]` | 18 tests |
| T11 — Codex-style patch format parser and applier | `[x]` | 18 tests |
| T12 — stygian-graph + stygian-browser integration for web fetching | `[x]` | 4 tests (stygian feature) |

---

## Phase 5 — Agent Engine

> Depends on: Phase 4 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T13 — ReplayLogger: JSONL delta-encoded LLM call log | `[x]` | 9 tests |
| T14 — Context condensation and turn summaries | `[x]` | 9 tests |
| T15 — RLMEngine: recursive tool-calling agent loop (application/services/ + CQRS command handler) | `[x]` | 13 tests |

---

## Phase 6 — Wiki Graph

> Depends on: Phase 5 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T16 — WikiGraphModel: index parsing, cross-ref extraction, petgraph DAG | `[x]` | 13 tests |

---

## Phase 7 — Session Persistence

> Depends on: Phase 6 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T17 — SQLite-backed session store (rusqlite) | `[x]` | 10 tests — can_write_session perm, Operator role for ownership-scoped tests |

---

## Phase 8 — Data Fetchers

> Depends on: Phase 7 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T18 — Fetcher trait, CLI entry points, and output conventions | `[x]` | 10 tests — FetchConfig+FetchOutput+FetchError, shared client w/ User-Agent, rate_limit_delay, NDJSON writer, FetcherArgs CLI |
| T19 — 12 ported fetcher binaries (FEC, SEC, USASpending, lobbying, OFAC, ICIJ, 990, Census, EPA, FDIC, OSHA, SAM) | `[x]` | 12 library modules in `fetchers/`, 12 unit tests (237 total) |
| T20 — 14 new fetcher binaries expanding corporate, sanctions, courts, and property intelligence | `[x]` | 14 library modules + 6 pipeline TOML configs, 19 unit tests (256 total) |
| T21 — 8 individual-person OSINT fetchers (HIBP, GitHub, Wayback, WHOIS/RDAP, voter rolls, USPTO, username enum, social profiles) | `[ ]` | |

---

## Phase 9 — TUI

> Depends on: Phase 8 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T22 — ratatui TUI: chat pane, wiki-graph canvas, activity indicator, REPL | `[ ]` | |

---

## Phase 10 — CLI Entry Point

> Depends on: Phase 9 all complete

| Task | Status | Notes |
| --- | --- | --- |
| T23 — clap CLI: run, tui, fetch, session, configure, version | `[ ]` | |

---

## Phase 11 — Integration and Polish

> Depends on: Phase 10 all complete

| Task | Status | Notes |
| --- | --- | --- |
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
- T02: `CredentialGuard<T>` with Deref + "***REDACTED***" Debug/Display is the zero-cost secret masking pattern. serde(transparent) keeps JSON clean.
- T03: Credential adapter uses only std I/O (no tokio/reqwest needed). set_owner_only_perms uses cfg(unix) platform gate.
- T03: Resolution order: explicit > env vars > .env file > workspace store > user store. merge_missing() fills only None fields.
- T04: PersistentSettings uses skip_serializing_if = "Option::is_none" for clean JSON. Unknown keys silently ignored (serde default).
- T05: SecurityPolicy trait is object-safe (`&dyn SecurityPolicy`). StaticPolicy uses explicit match arms per role — no role ordering/comparison.
- T05: UserId newtype wraps Uuid, no Copy — forces intentional passing. UserId::system() returns nil UUID.
- T05: AuthContext carries `CredentialGuard<String>` for session_token — redacted in Debug output. All downstream CQRS types needed zero changes (AuthContext used opaquely).
- T06: SSE parsing uses byte-level parse_sse_events() → StreamAccumulator pattern. Tool-call JSON fragments accumulate via InProgressToolCall vec, then joined and parsed.
- T06: Thinking budgets only for models matching `contains("opus-4")` — use let-chain to collapse nested ifs. All Claude models currently share 200k context window.
- T06: CustomDebug on AnthropicModel excludes api_key field — CredentialGuard alone isn't enough since the struct holds it as a field.
- T07: Single OpenAICompatibleModel serves OpenAI/OpenRouter/Cerebras/Ollama via for_provider() factory. OpenRouter needs HTTP-Referer + X-Title headers. Ollama gets 120s timeout.
- T07: OpenAI SSE uses `data: [DONE]` terminator — must filter before JSON parse. Tool call deltas indexed by `index` field, not by content_block events like Anthropic.
- T08: ModelProvider uses RPITIT so it's NOT dyn-compatible. Use ProviderBox enum-dispatch instead of `Arc<dyn ModelProvider>`. Clippy enforces async fn over impl Future for simple delegation (manual_async_fn lint).
- T08: Ollama doesn't require an API key — use empty CredentialGuard placeholder. Judge model prefers claude-haiku-4-5, falls back to gpt-4o-mini.
- T08: list_models endpoint format differs: Ollama uses `{"models": [...]}` with `name` field; OpenAI/Anthropic use `{"data": [...]}` with `id` field. Anthropic needs `anthropic-version` header.
- T09: Tool definitions live in adapters/tool_defs.rs (provider-specific converters). 18 base tools + 2 delegation tools (subtask/execute) gated on recursive flag.
- T09: to_anthropic_tools() uses `input_schema` key; to_openai_tools() wraps in `{type: "function", function: {name, description, parameters}}`. All parameters have type=object + additionalProperties=false.
- T10: WorkspaceTools in adapters/tools/ (mod+filesystem+shell+web) implements ToolDispatcher. resolve_path() canonicalizes and checks workspace containment. Pre-read write guard via files_read HashSet.
- T10: Shell policy: HEREDOC_RE and INTERACTIVE_RE block dangerous/interactive commands. tokio::time::timeout + wait_with_output for clean timeout handling (wait_with_output takes ownership; restructure code so timeout drops the future which drops child).
- T10: crc32fast for hashline hashes (2-char hex, whitespace-invariant). regex for symbol extraction in repo_map. base64 for read_image. All behind `runtime` feature.
- T10: Parallel write conflict detection via WriteGroup with group_id→(path→owner_id) claims map. scope_group_id/scope_owner_id set per execution context.
- T10: Edition 2024 let-chains: `if let Some(g) = glob && !fnmatch(...)` collapses nested ifs. Use `&Path` not `&PathBuf` for function params (clippy ptr_arg).
- T11: Patching module in adapters/tools/patching.rs. Parser splits on `*** Add File:`, `*** Delete File:`, `*** Update File:` lines. Chunks split on `@@` separators.
- T11: Two-pass hunk matching: exact then whitespace-normalised (collapse runs to single space). Cursor advances after each chunk for ordered multi-hunk application.
- T11: ApplyReport returned even on partial failure — each operation is independent. resolve closure enforces workspace path safety.
- T11: apply_patch in filesystem.rs delegates to patching::apply_patch and marks all added/updated/moved files as read for subsequent edits.
- T12: Stygian integration behind `stygian` feature flag. BrowserPool lazy via `Arc<OnceCell<Arc<BrowserPool>>>`. StygianIntegration, fetch_url_smart, is_likely_spa, run_scrape_pipeline.
- T12: stygian-browser 0.1: `BrowserPool::new(config) -> Result<Arc<Self>>`, `pool.acquire() -> Result<BrowserHandle>`, `WaitUntil::NetworkIdle` is a unit variant (no fields).
- T12: stygian-graph 0.1: `PipelineUnvalidated::new(config).validate()?.execute().complete(results)` typestate pattern. Empty pipeline validation may fail — don't assert Ok.
- T12: Compiles and passes tests with AND without `stygian` feature. `run_scrape_pipeline` dispatch returns "requires stygian feature" when disabled.
- T13: FileReplayLogger in adapters/persistence/replay_log.rs implements ReplayLog port. Uses AtomicU32 (seq) + AtomicUsize (prev_len) for thread-safe delta encoding. HeaderParams/CallParams structs avoid clippy too_many_arguments.
- T13: Delta encoding: seq 0 → full messages_snapshot; seq N → messages_delta (slice from prev_len). File opened in append mode (never truncated). JSONL = one complete JSON object per line.
- T13: Child loggers via `child(depth, step)` produce hierarchical IDs: `root/d{depth}s{step}`. Grandchild nests: `root/d2s5/d3s1`. All append to same JSONL file.
- T14: ContextTracker in application/services/condensation.rs. should_condense() triggers when used_tokens > 75% of window_size. condense_tool_outputs() replaces old tool-role messages with placeholder, keeping last 4.
- T14: condense_with_judge() asks judge model for 400-word summary, rebuilds conversation: objective + <condensation> turn + recent messages. Preserves first user message (objective) always.
- T14: model_context_window() heuristic: claude* = 200k, gpt-4.1/gpt-4o/o4-mini = 128k, cerebras/llama = 128k, default 128k. services.rs converted to module file for services/ directory.
- T15: RLMEngine<M, D, R> in application/services/engine.rs is generic over ModelProvider, ToolDispatcher, ReplayLog (all RPITIT traits). Uses CancellationToken (tokio-util) behind cfg(feature = "runtime").
- T15: solve_recursive returns Pin<Box<dyn Future + Send>> for recursive async indirection. handle_subtask recurses via solve_recursive(depth+1). max_depth guards prevent infinite recursion.
- T15: Runtime policy: shell_command_counts HashMap<(depth, cmd_str), count> blocks identical run_shell after 2 repeats per depth level. Uses Mutex for interior mutability through &self.
- T15: ExternalContext accumulates cross-turn observations with add() and summary(max_items, max_chars). Passed as &mut through recursive calls.
- T15: Test infrastructure: ScriptedModel (pre-programmed ModelTurns via AtomicUsize index), ScriptedDispatcher (pre-programmed results), NoopReplayLog. make_engine() helper constructs with default AgentConfig.
- T16: WikiGraphModel in adapters/wiki_graph.rs with petgraph DiGraph<WikiNode, WikiEdge>. parse_index reads `### Category` headers and `| Name | Jurisdiction | [link](path) |` rows. extract_cross_refs finds bold `**text**` in `## Cross-Reference Potential` sections.
- T16: Name registry maps lowered variants → NodeIndex: full name, title, parenthetical contents, slash-split parts, file slug. match_reference uses 5-stage lookup: exact → parenthetical → substring → token overlap (≥2) → jaro_winkler (≥0.88).
- T16: Hand-rolled jaro/jaro_winkler to avoid strsim dep. WikiWatcher uses tokio interval poll loop (Send+Sync, non-blocking) with CancellationToken instead of notify crate.
