# T10 — WorkspaceTools: filesystem, shell, web, and parallel-write safety (adapters/tools/)

> **Depends on**: T-tool-defs, T-credentials, T-security-model.

## Goal

Implement all 19 tools split across redshank-core/src/adapters/tools/ modules,
mirroring agent/tools.py: filesystem.rs (list_files, read_file, write_file, edit_file,
hashline_edit, read_image), shell.rs (run_shell, run_shell_bg, check/kill/cleanup_bg_jobs),
web.rs (web_search, fetch_url), patching.rs (apply_patch), stygian.rs (run_scrape_pipeline).
WorkspaceTools implements the ToolDispatcher port from src/ports/tool_dispatcher.rs.
All tool dispatch requires &AuthContext — operators and above may dispatch tools;
readers may not call write or shell tools.


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

- Test: write_file to a path outside workspace returns PathEscape error.
- Test: write_file to an unread path returns PreReadGuard error.
- Test: run_shell with heredoc syntax returns Heredoc error.
- Test: run_shell timeout kills the process within 200ms of deadline.
- Test: run_shell_bg registers job; check_shell_bg returns running status; kill_shell_bg terminates it.
- Test: web_search constructs correct JSON body and parses mock response.
- Test: parallel write group blocks write from a non-owning branch.


### 2. GREEN — Implement to pass

- WorkspaceTools (adapters/tools/mod.rs, implements ToolDispatcher port): { workspace: PathBuf, creds: CredentialBundle, bg_jobs: Arc<Mutex<HashMap<String, BgJob>>>, write_group: Arc<Mutex<Option<WriteGroup>>> }. ToolDispatcher::dispatch(call: &ToolCall, auth: &AuthContext) -> ToolResult routes to the correct submodule; checks can_run_agent(auth, policy) — returns ToolResult { is_error: true, content: AccessDenied message } if denied.
- Path-escape guard: canonicalize path and assert it starts with workspace root; return ToolError::PathEscape otherwise.
- Pre-read write guard: write_file() checks a per-path read-set (thread-local or per-session); blocks writes to files not yet read in the current session unless force=true.
- Heredoc blocking: run_shell() scans command string for '<<' or 'EOF' and returns ToolError::Heredoc if found.
- Interactive blocking: block commands containing 'vim', 'nano', 'less', 'more', 'top', 'htop' etc.
- run_shell(): spawn with tokio::process::Command, SIGKILL on timeout (command_timeout_sec from config), capture stdout+stderr, prepend hashline prefix to output.
- run_shell_bg(): spawn detached, register in bg_jobs map with UUID key, return job ID.
- web_search(): POST to Exa API (https://api.exa.ai/search) with exa_api_key from CredentialBundle.
- fetch_url(): reqwest GET, follow redirects, return body text truncated to max_observation_chars.
- hashline_edit(): parse the hashline-prefixed read output to locate target lines; apply edit.
- read_image(): read file bytes, base64-encode, return as ImageData { mime_type, data }.
- begin/end_parallel_write_group(): track ownership of file paths per parallel branch; block cross-branch writes.


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

Update PROGRESS.md row for T10 to `[x]`.
Commit: `feat(workspace-tools): implement workspacetools: filesystem, shell, web, and parallel-write safety (adapters/tools/)`
