# T23 — clap CLI: run, tui, fetch, session, configure, version

> ⛔ **GATE — Human confirmation required before starting this task.**
> Run `cargo test --workspace` locally and confirm all phase 1–9 tests pass before starting.

> **Depends on**: T-tui-ratatui, T-session-store, T-fetchers-extended, T-credentials, T-settings.

## Goal

Wire the binary entry point using clap derive. Subcommands: run (headless),
tui (interactive TUI), fetch <source>, session list/delete/resume, configure
(interactive credential setup), version.


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

- CLI integration test: redshank version prints semver.
- CLI integration test: redshank configure with mocked stdin saves credentials JSON.
- CLI integration test: redshank session list with empty DB prints empty table (not an error).


### 2. GREEN — Implement to pass

- run <objective>: load config+creds, build session, call SessionRuntime.solve(), print result to stdout.
- tui: launch ratatui TUI, load last session or create new.
- fetch <source> [--output <dir>] [--query <str>]: dispatch to the named fetcher binary.
- session list: print table of sessions (ID, created, last objective).
- session delete <id>: delete session from DB.
- session resume <id>: resume an existing session in TUI or headless mode.
- configure: interactive credential setup (rpassword prompts), saves to ~/.redshank/credentials.json.
- version: print semver + git SHA (embed with env!(CARGO_PKG_VERSION) and a build.rs git SHA).
- Global flags: --workspace <path>, --model <name>, --reasoning <effort>, --no-tui, --max-depth <n>, --demo.
- Structured JSON logging via tracing + tracing-subscriber; default INFO, RUST_LOG override.
- Startup log line: 'redshank v{VERSION} ({GIT_SHA}) — model: {model} — workspace: {path}'.


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

Update PROGRESS.md row for T23 to `[x]`.
Commit: `feat(cli-entrypoint): implement clap cli: run, tui, fetch, session, configure, version`
