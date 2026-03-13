# T17 — SQLite-backed session store (rusqlite)

> **Depends on**: T-domain-types, T-wiki-graph.

## Goal

Implement the SessionStore port using rusqlite. Replace OpenPlanter's scattered
JSONL + JSON files with a single .redshank/sessions.db SQLite database.
Schema: sessions, turns, events, artifacts, observations.


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

- Test: create_session with AuthContext::system() round-trips metadata through load_session.
- Test: load_session with mismatched user_id returns DomainError::Security(AccessDenied).
- Test: list_sessions returns only sessions owned by the requesting user_id.
- Test: list_sessions with Owner role returns all sessions.
- Test: update_session closure returning (true, value) persists the mutation; (false, value) rolls back.
- Test: append_event seq numbers increment across calls; list_events returns them in order.
- Test: check_idempotency_key returns None on first call, cached result on second call.
- Test: idempotency keys older than 24 hours return None.
- Test: wiki seed copies files; second session creation does not overwrite agent-modified file.
- Test: WAL mode confirmed (PRAGMA journal_mode = WAL returns 'wal').
- Test: delete_session removes all related rows (CASCADE) and requires can_delete_session permission.


### 2. GREEN — Implement to pass

- SqliteSessionStore lives in src/adapters/persistence/sqlite.rs and implements the SessionStore port from src/ports/session_store.rs. SessionRuntime lives in src/application/services/session_runtime.rs.
- Tables: sessions (id TEXT PK, created_at, owner_user_id TEXT NOT NULL, metadata JSON), turns (id, session_id FK, turn_index, summary JSON), events (id, session_id FK, seq INTEGER, event_type TEXT, payload JSON, ts), artifacts (id, session_id FK, name TEXT, path TEXT), observations (id, session_id FK, content TEXT, ts), idempotency_keys (key TEXT PK, session_id TEXT, result JSON, created_at INTEGER). Run CREATE TABLE IF NOT EXISTS at open time — no migration runner.
- Use rusqlite in WAL mode (PRAGMA journal_mode = WAL) for concurrent readers.
- Security-first port: every method accepts auth: &AuthContext. create_session checks can_run_agent; load_session checks can_read_session and verifies auth.user_id == session.owner_user_id unless Role::Owner/Service; list_sessions returns only sessions owned by the requesting user (or all if Owner/Service); delete_session checks can_delete_session. No bypasses.
- UpdateFn pattern for transactional mutations (from docs/dev/database_transactions_summary.md): async fn update_session<F, R>(&self, id: &SessionId, auth: &AuthContext, update_fn: F) -> Result<R> where F: FnOnce(&mut AgentSession) -> Result<(bool, R), DomainError> + Send, R: Send. Closure returns (should_save, return_value); repo manages the SQLite transaction.
- Idempotency: check_idempotency_key(key: &IdempotencyKey) -> Option<serde_json::Value> — SELECT from idempotency_keys WHERE key=? AND created_at > now()-86400. mark_idempotency_key(key, result) — INSERT OR REPLACE.
- Domain event persistence: append_event(auth, session_id, event: &DomainEvent) serialises to JSON and INSERTs into events with auto-incrementing seq. list_events(auth, session_id) returns events ORDER BY seq ASC.
- WikiSeed: on session creation, copy wiki/ baseline into .redshank/wiki/ (never overwrite agent-modified files — track via seed_applied flag in sessions table).
- SessionRuntime (application/services/session_runtime.rs): wraps RunInvestigationHandler + WikiSeed; on each step appends DomainEvents from AgentSession::drain_events() via append_event, persists TurnSummary via save_turn.
- Aggregate repository: SessionStore is the single repository for the AgentSession aggregate root — all related data (turns, events, artifacts, observations) flows through this one port.


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

Update PROGRESS.md row for T17 to `[x]`.
Commit: `feat(session-store): implement sqlite-backed session store (rusqlite)`
