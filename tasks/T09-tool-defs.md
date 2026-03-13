# T09 — Tool definitions: JSON schemas and to_provider() converters

> **Depends on**: T-domain-types.

## Goal

Define TOOL_DEFINITIONS as a static list of ToolDefinition structs covering all
19 tools. Implement to_anthropic_tools() and to_openai_tools() converters.


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

- Test: TOOL_DEFINITIONS contains exactly 20 entries (19 + group tools) when recursive=true.
- Test: to_openai_tools() output has {type: 'function'} wrapper on every entry.
- Test: subtask absent when recursive=false.


### 2. GREEN — Implement to pass

- Tools (19 total): list_files, search_files, repo_map, read_file, write_file, edit_file, apply_patch, hashline_edit, read_image, run_shell, run_shell_bg, check_shell_bg, kill_shell_bg, cleanup_bg_jobs, web_search, fetch_url, subtask, execute, begin_parallel_write_group, end_parallel_write_group.
- ToolDefinition { name, description, parameters: serde_json::Value (JSON Schema object) }.
- subtask and execute are only included when AgentConfig.recursive == true.
- to_anthropic_tools() wraps each as {name, description, input_schema: parameters}.
- to_openai_tools() wraps each as {type:'function', function:{name,description,parameters}}.


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

Update PROGRESS.md row for T09 to `[x]`.
Commit: `feat(tool-defs): implement tool definitions: json schemas and to_provider() converters`
