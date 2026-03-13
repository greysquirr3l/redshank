# T25 — Coraline MCP tool bindings for self-directed code navigation

> **Depends on**: T-tool-defs, T-workspace-tools.

## Goal

When the agent is directed at the redshank workspace itself, it should be able
to use Coraline MCP tools (coraline_read_file, coraline_search, coraline_repo_map,
coraline_edit_file) via the tool layer. Wire these as additional ToolDefinitions
behind a 'coraline' feature flag.


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

- Test: coraline_* tools absent when feature is disabled.
- Test: coraline_* tools present in TOOL_DEFINITIONS when feature is enabled.


### 2. GREEN — Implement to pass

- coraline feature: if 'coraline' feature is enabled, add four extra ToolDefinitions: coraline_read_file, coraline_search, coraline_repo_map, coraline_edit_file.
- These tools proxy to the Coraline MCP server over stdio (use the coraline crate's MCP client).
- WorkspaceTools::dispatch() routes coraline_* calls to the MCP proxy.
- The orchestrator rules already state to use Coraline for code navigation — these tools make that concrete.


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

Update PROGRESS.md row for T25 to `[x]`.
Commit: `feat(coraline-mcp-tools): implement coraline mcp tool bindings for self-directed code navigation`
