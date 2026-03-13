# AGENTS.md

## Project

redshank — Redshank is an autonomous recursive language-model investigation agent written
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


## Setup commands

- Build: `cargo build --workspace`
- Test: `cargo test --workspace`
- Lint: `cargo clippy --workspace -- -D warnings`

## Code style

- Language: rust
- Strategy: TDD — write a failing test before any implementation code


## Rules

- Rust edition 2024, stable toolchain 1.94 only — no nightly features.

- All error types use thiserror; no .unwrap() or .expect() outside of tests and main().

- The domain layer (redshank-core/src/domain/) must have zero I/O dependencies: no tokio, no reqwest, no sqlx. Enforced by the [dependencies] section of redshank-core having no I/O crates as non-optional dependencies.

- Async runtime is tokio with the full feature set.

- Use serde + serde_json for all serialisation; derive Serialize/Deserialize on every domain type.

- Credentials are stored chmod 600; never appear in log output at any level.

- Use coraline MCP tools (coraline_read_file, coraline_search, etc.) when exploring the workspace.

- All public API items must have rustdoc comments.

- Keep stygian-graph and stygian-browser behind feature flags so the binary can be built without a Chrome install.

- Write tests before or alongside implementation (TDD strategy).

- Mirror OpenPlanter's existing test coverage: engine loop, tools, model layer, patching, credentials, session, wiki-graph, TUI events, and all data-fetcher scripts.

- Internal directory structure for every crate mirrors stygian-graph/src/: src/domain/ (zero I/O pure types and aggregates), src/ports/ (trait interfaces — inbound + outbound), src/application/commands/ (CQRS mutating handlers), src/application/queries/ (CQRS read-only handlers), src/application/services/ (orchestration), src/adapters/providers/, src/adapters/tools/, src/adapters/persistence/. Top-level src/{domain,ports,application,adapters}.rs re-export the subtree.

- CQRS: every mutating operation is a Command struct (carries IdempotencyKey: newtype Uuid v4) handled by a CommandHandler in application/commands/. Every read operation is a Query struct handled by a QueryHandler in application/queries/. Commands are idempotent — handlers check an idempotency_keys table before executing and return the cached result on duplicate.

- Security First (fail-secure): every repository/store port method that accesses or mutates keyed data accepts auth: &AuthContext and enforces a SecurityPolicy check before any data access. Security rules live in src/domain/auth.rs as pure functions (no I/O, no async). Default deny — return Err(SecurityError::AccessDenied) unless the policy explicitly grants the required Permission.

- Aggregate repositories: one repository per aggregate root, not one per table. Use the UpdateFn pattern for transactional mutations: async fn update_by_id<F, R>(&self, id, auth: &AuthContext, update_fn: F) -> Result<R>. The closure holds business logic; the repo manages the transaction. Use TransactionProvider only for cross-aggregate consistency.

- Domain events: every significant state transition emits a typed DomainEvent variant (SessionCreated, AgentStarted, ToolCalled, AgentCompleted, WikiEntryWritten). Events are immutable value types. Aggregate methods append them to a pending_events Vec; the session store persists them via append_event.

- Idempotency: all CommandHandlers check and set idempotency_keys via the SessionStore port. Duplicate commands (same IdempotencyKey) return the stored result without re-executing.

- No domain type may reference an adapter or application type. Adapters implement port traits. Port traits reference only domain types.

## Testing instructions

- Run `cargo test --workspace` before committing
- Every new public function needs at least one test
- Fix all test failures before marking a task complete

## Commit conventions

- Use conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`
- Focus commit messages on user impact, not file counts or line numbers

---

_Generated by [wiggum](https://github.com/greysquirr3l/wiggum)._
