# T08 — Provider builder and model-name inference

> **Depends on**: T-provider-anthropic, T-provider-openai-compat.

## Goal

Implement build_provider() factory: infers ProviderKind from model name,
constructs the right ModelProvider impl, and wraps it in Arc<dyn ModelProvider>.
Also implement list_models() for each provider.


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

- Test: infer_provider('claude-opus-4-6') == ProviderKind::Anthropic.
- Test: infer_provider('gpt-5.2') == ProviderKind::OpenAI.
- Test: build_provider() returns Err when required API key is missing.


### 2. GREEN — Implement to pass

- infer_provider(model: &str) -> ProviderKind: regex-based: /^claude-/ → Anthropic, /^gpt-|^o[0-9]/ → OpenAI, /^llama|^mistral|^qwen/ → Ollama (or OpenRouter if key set).
- build_provider() accepts AgentConfig + CredentialBundle.
- build_judge_model() builds a cheap model (claude-haiku-4-5 or gpt-4o-mini) for acceptance-criteria evaluation.
- list_models(provider, creds) → Vec<String>: GET /models from each provider API.


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

Update PROGRESS.md row for T08 to `[x]`.
Commit: `feat(provider-builder): implement provider builder and model-name inference`
