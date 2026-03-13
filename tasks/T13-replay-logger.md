# T13 — ReplayLogger: JSONL delta-encoded LLM call log

> **Depends on**: T-domain-types.

## Goal

Port agent/replay_log.py to Rust: append JSONL records for every LLM call
using delta encoding (seq 0 = full snapshot; seq N = delta since seq N-1).
Child loggers for subtasks use hierarchical IDs (root/d2s5).


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

- Test: seq 0 log line contains full messages snapshot.
- Test: seq 1 line contains only the two new messages appended.
- Test: child logger ID is 'root/d2s5' when parent is 'root' and label is 'd2s5'.
- Test: JSONL file is valid (each line parses independently).


### 2. GREEN — Implement to pass

- ReplayLogger { path: PathBuf, seq: AtomicU32, parent_id: Option<String> }.
- log_call(session_id, messages_snapshot, response) appends a JSON line with { session_id, seq, timestamp, delta_messages, response }.
- Delta: seq 0 stores full messages Vec; seq N stores only the suffix appended since seq N-1 (tracked as prev_len).
- child(subtask_label) -> ReplayLogger: appends to same file, ID = parent_id + '/' + subtask_label.
- File opened in append mode (OpenOptions::new().append(true).create(true)).


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

Update PROGRESS.md row for T13 to `[x]`.
Commit: `feat(replay-logger): implement replaylogger: jsonl delta-encoded llm call log`
