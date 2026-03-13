# T03 — Credential bundle, storage (chmod 600), and resolution order

> **Depends on**: T-domain-types.

## Goal

Implement the CredentialBundle (7 keys), credential stores (workspace + user-level),
.env file parser, and multi-source merge — mirroring agent/credentials.py.


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

- Test: .env file parser handles KEY=value, KEY='value', KEY="value", #comments, blank lines.
- Test: resolution order — explicit arg wins over env var wins over file.
- Test: has_any() returns false when all fields are None.
- Test: written credential file has mode 0o600 on Linux.


### 2. GREEN — Implement to pass

- CredentialBundle fields (all Option<CredentialGuard<String>> — see security-model task for CredentialGuard spec): openai_api_key, anthropic_api_key, openrouter_api_key, cerebras_api_key, exa_api_key, voyage_api_key, ollama_base_url, hibp_api_key, github_token.
- Resolution order (highest wins): (1) explicit CLI args, (2) OPENPLANTER_* env vars or bare provider env vars, (3) .env file in workspace, (4) .redshank/credentials.json in workspace, (5) ~/.redshank/credentials.json user-level.
- Write credentials JSON with std::fs::set_permissions (mode 0o600) immediately after writing.
- Interactive configure_keys() function: prompts each key with rpassword, saves to user store.
- has_any() returns true if at least one key is set.
- merge_missing() fills in empty fields from a lower-priority bundle.


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

Update PROGRESS.md row for T03 to `[x]`.
Commit: `feat(credentials): implement credential bundle, storage (chmod 600), and resolution order`
