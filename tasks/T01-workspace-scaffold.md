# T01 — Cargo workspace, CI, and repo hygiene

> **Depends on**: None.

## Goal

Bootstrap a compilable Cargo workspace with stub crates, GitHub Actions CI,
clippy config, deny.toml, and edition 2024 throughout.


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

- CI green: cargo build && cargo clippy && cargo test all pass on a clean checkout.
- Test: redshank-core/src/domain/ has no tokio, reqwest, sqlx, or any I/O crate in its transitive dependencies (verify with cargo tree -p redshank-core --edges no-dev).
- Test: each top-level module file (domain.rs, ports.rs, application.rs, adapters.rs) re-exports all public items from its subdirectory.


### 2. GREEN — Implement to pass

- Workspace members: redshank-core, redshank-tui, redshank-fetchers, redshank-cli. Four crates replace the original seven — all domain/provider/tool/engine concerns are unified in redshank-core, which follows the stygian-graph src/ layout exactly.
- redshank-core/src/ layout (mirrors https://github.com/greysquirr3l/stygian/tree/main/crates/stygian-graph/src): domain.rs + domain/ (zero I/O), ports.rs + ports/ (trait interfaces), application.rs + application/ (CQRS + services), adapters.rs + adapters/ (LLM providers, tools, persistence), lib.rs.
- redshank-core/src/domain/ files: agent.rs (AgentConfig value object, AgentSession aggregate root), credentials.rs (CredentialBundle, CredentialGuard<T> newtype), events.rs (DomainEvent enum), session.rs (Session, SessionId, TurnSummary, ToolCall, ToolResult, ModelTurn, StopReason), wiki.rs (WikiEntry, WikiCategory), auth.rs (AuthContext, Permission, Role, SecurityPolicy trait, SecurityError), errors.rs (DomainError hierarchy).
- redshank-core/src/ports/ files: model_provider.rs (ModelProvider: async complete + count_tokens), tool_dispatcher.rs (ToolDispatcher: async dispatch takes &AuthContext), session_store.rs (SessionStore — all methods include auth: &AuthContext + idempotency methods), wiki_store.rs (WikiStore), fetcher.rs (DataFetcher), replay_log.rs (ReplayLog). All traits object-safe.
- redshank-core/src/application/ layout: commands/ (run_investigation.rs, configure_credentials.rs, delete_session.rs — each with Command struct + IdempotencyKey + Handler impl), queries/ (get_session.rs, list_sessions.rs, get_wiki_entry.rs — each with Query struct + Handler impl), services/ (engine.rs — RLMEngine, condensation.rs — ContextTracker, session_runtime.rs — SessionRuntime).
- redshank-core/src/adapters/ layout: providers/ (anthropic.rs, openai_compat.rs, builder.rs), tools/ (filesystem.rs, shell.rs, web.rs, patching.rs, stygian.rs — WorkspaceTools implements ToolDispatcher port), persistence/ (sqlite.rs — SqliteSessionStore implements SessionStore, wiki_fs.rs — FsWikiStore implements WikiStore, replay_log.rs — FileReplayLogger implements ReplayLog).
- redshank-tui/src/ layout: domain/ (AppState, AppEvent, UiCommand value types), application/ (event_loop.rs, slash_commands.rs with SlashCommand as CQRS command variants), adapters/ (renderer.rs using ratatui, crossterm_reader.rs), lib.rs.
- redshank-fetchers/src/ layout: domain/ (FetchConfig, FetchOutput), ports/ (HttpClientPort trait), application/ (one FetchQuery handler per data source), adapters/ (http.rs, browser.rs, per-source adapter modules), bin/ (one binary entry point per fetcher).
- redshank-cli/src/main.rs: thin clap derive CLI only — constructs Command/Query structs from args and dispatches to redshank-core handlers. Zero business logic in the CLI layer.
- Each crate gets a stub lib.rs with a single #[test] fn it_compiles() { }.
- GitHub Actions: cargo build + clippy -D warnings + cargo test on ubuntu-latest.
- Add deny.toml (cargo-deny) for supply-chain gate.
- Root Cargo.toml: [workspace] resolver = '2', edition = '2024' in each member.
- .cargo/config.toml: set RUSTFLAGS = '-D warnings' so CI and local are identical.


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

Update PROGRESS.md row for T01 to `[x]`.
Commit: `feat(workspace-scaffold): implement cargo workspace, ci, and repo hygiene`
