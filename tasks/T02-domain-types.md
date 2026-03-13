# T02 — Domain model: entities, value objects, aggregates, domain events, and CQRS command/query types

> **Depends on**: T-workspace-scaffold.

## Goal

Implement the pure domain layer in redshank-core/src/domain/ following DDD-Lite principles:
aggregate roots (AgentSession), value objects (AgentConfig, CredentialBundle), domain events
(DomainEvent enum), and the type vocabulary shared by every crate. Also stub the CQRS command
and query structs in application/commands/ and application/queries/.
Port trait interfaces (ModelProvider, SessionStore, etc.) live in src/ports/ so the domain
has zero knowledge of how they are implemented — see the separate hints below.


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

- Unit test: AgentConfig::default() has expected field values.
- Unit test: ProviderKind::from_model_name() correctly infers Anthropic for 'claude-*', OpenAI for 'gpt-*', etc.
- Unit test: all domain types round-trip through serde_json without loss.
- Unit test: AgentSession::create() appends SessionCreated to pending_events.
- Unit test: AgentSession::drain_events() clears pending_events after returning them.
- Unit test: DomainEvent variants all carry session_id and timestamp fields.
- Unit test: CredentialGuard Debug output is '***REDACTED***' regardless of inner value.
- Unit test: RunInvestigationCommand round-trips through serde_json.


### 2. GREEN — Implement to pass

- AgentConfig (value object): workspace (PathBuf), provider (ProviderKind), model (String), reasoning_effort (ReasoningEffort), max_depth (u8), max_steps (u32), max_observation_chars (usize), command_timeout (Duration), max_file_chars (usize), recursive (bool), acceptance_criteria (bool), demo_mode (bool). ProviderKind: Anthropic, OpenAI, OpenRouter, Cerebras, Ollama. ReasoningEffort: None, Low, Medium, High.
- AgentSession (aggregate root in domain/agent.rs): session_id (SessionId), config (AgentConfig), status (SessionStatus: Idle, Running, Completed, Failed), turns (Vec<TurnSummary>), pending_events (Vec<DomainEvent>). Methods: AgentSession::create(config) emits SessionCreated; start(objective) emits AgentStarted; complete(result) emits AgentCompleted; add_turn(summary, tool_names) emits ToolCalled per tool; drain_events() -> Vec<DomainEvent> (takes and clears pending_events).
- SessionId: newtype around Uuid. IdempotencyKey: newtype around Uuid v4 (used by CQRS commands).
- DomainEvent enum in domain/events.rs (all variants carry session_id: SessionId and timestamp: DateTime<Utc>): SessionCreated { session_id, config: AgentConfig, timestamp }, AgentStarted { session_id, objective: String, timestamp }, ToolCalled { session_id, tool_name: String, args_summary: String, timestamp }, AgentCompleted { session_id, result_summary: String, timestamp }, WikiEntryWritten { session_id, entry_path: PathBuf, category: WikiCategory, timestamp }. Events are immutable value types — no mutation methods.
- ToolCall { id: String, name: String, arguments: serde_json::Value }. ToolResult { call_id: String, content: String, is_error: bool }. ModelTurn { content: Option<String>, tool_calls: Vec<ToolCall>, stop_reason: StopReason }. StopReason: EndTurn, ToolUse, MaxTokens, StopSequence. TurnSummary { turn: u32, summary: String, tool_names: Vec<String>, timestamp: DateTime<Utc> }.
- WikiEntry { path: PathBuf, title: String, category: WikiCategory, cross_refs: Vec<String> }. WikiCategory: CampaignFinance, Contracts, Corporate, Financial, Infrastructure, International, Lobbying, Nonprofits, People, Other. — People covers individual-person OSINT entries (identity profiles, breach exposure summaries, username maps, voter records).
- CredentialBundle (value object, all fields Option<CredentialGuard<String>>): openai_api_key, anthropic_api_key, openrouter_api_key, cerebras_api_key, exa_api_key, voyage_api_key, ollama_base_url, hibp_api_key, github_token. CredentialGuard<T>: newtype wrapping T; Debug/Display → '***REDACTED***' (manual impl, NOT derived); Deref<Target=T>; Serialize delegates to T; Deserialize wraps T. Never derives Debug automatically.
- AuthContext, Permission, Role, SecurityPolicy, and SecurityError are defined in domain/auth.rs — full specification is in the 'security-model' task in this phase.
- Port traits live in src/ports/, NOT in src/domain/. ModelProvider (ports/model_provider.rs): async fn complete(messages) -> Result<ModelTurn>; async fn count_tokens(messages) -> Result<u32>. ToolDispatcher (ports/tool_dispatcher.rs): async fn dispatch(call: &ToolCall, auth: &AuthContext) -> ToolResult. SessionStore (ports/session_store.rs): all methods include auth: &AuthContext; includes check_idempotency_key and mark_idempotency_key. WikiStore (ports/wiki_store.rs). DataFetcher (ports/fetcher.rs). ReplayLog (ports/replay_log.rs). All traits object-safe — no generic methods on the trait itself.
- CQRS command value types in src/application/commands/: RunInvestigationCommand { idempotency_key: IdempotencyKey, session_id: SessionId, objective: String, config: AgentConfig, auth: AuthContext }. ConfigureCredentialsCommand { idempotency_key, credentials: CredentialBundle, auth: AuthContext }. DeleteSessionCommand { idempotency_key, session_id, auth: AuthContext }.
- CQRS query value types in src/application/queries/: GetSessionQuery { session_id, auth: AuthContext }. ListSessionsQuery { auth: AuthContext }. GetWikiEntryQuery { path: PathBuf, auth: AuthContext }.


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

Update PROGRESS.md row for T02 to `[x]`.
Commit: `feat(domain-types): implement domain model: entities, value objects, aggregates, domain events, and cqrs command/query types`
