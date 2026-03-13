# T24 — Full-stack integration tests with scripted model

> **Depends on**: T-cli-entrypoint.

## Goal

Write end-to-end integration tests that exercise the full stack
(CLI → SessionRuntime → RLMEngine → WorkspaceTools) using a ScriptedModel
fixture — no live API calls.


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

- All integration tests use in-memory SQLite and a temp workspace dir.
- No live API or network calls in any integration test — use mock HTTP server (wiremock) for fetcher tests.


### 2. GREEN — Implement to pass

- ScriptedModel: implements ModelProvider; replays a pre-defined sequence of ModelTurn responses when complete() is called.
- Test: multi-turn session that writes a file, reads it back, runs a shell command, and returns a final answer — assert final text and file contents.
- Test: subtask delegation creates child session entry in DB.
- Test: context condensation triggers at 76% token usage in a scripted long conversation.
- Test: demo mode (--demo) censors entity names in output.
- Test: wiki seed creates .redshank/wiki/ on first session; second session leaves agent-modified file untouched.
- Test user stories (mirrors test_user_stories.py): follow-the-money scripted scenario writes expected wiki entries.


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

Update PROGRESS.md row for T24 to `[x]`.
Commit: `feat(integration-tests): implement full-stack integration tests with scripted model`
