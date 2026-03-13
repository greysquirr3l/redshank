# T11 — Codex-style patch format parser and applier

> **Depends on**: T-workspace-tools.

## Goal

Port agent/patching.py to Rust: parse *** Begin Patch / *** End Patch blocks
with Add File, Delete File, and Update File hunks. Two-pass matching: exact
then whitespace-normalised. Return ApplyReport.


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

- Test: Add File creates the file with correct content.
- Test: Delete File removes the file.
- Test: Update File with a single hunk replaces the matching lines.
- Test: Update File whitespace-normalised match succeeds when exact fails.
- Test: multi-hunk patch applies all hunks in order.
- Test: patch referencing a path outside workspace is rejected.


### 2. GREEN — Implement to pass

- Patch format: *** Begin Patch\n[operations]\n*** End Patch
- Operations: '*** Add File: <path>\n<content>', '*** Delete File: <path>', '*** Update File: <path>\n[hunks]'.
- Hunk lines: ' ' (context), '-' (remove), '+' (add).
- Two-pass hunk matching: first try byte-exact; if no match, try after stripping leading/trailing whitespace from each line.
- ApplyReport { added: Vec<PathBuf>, deleted: Vec<PathBuf>, updated: Vec<PathBuf>, errors: Vec<PatchError> }.
- apply_patch() in WorkspaceTools takes the raw patch string and calls the parser + applier, respecting workspace path safety.


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

Update PROGRESS.md row for T11 to `[x]`.
Commit: `feat(patching): implement codex-style patch format parser and applier`
