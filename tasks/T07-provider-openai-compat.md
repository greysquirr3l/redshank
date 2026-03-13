# T07 — OpenAICompatibleModel: OpenAI, OpenRouter, Cerebras, Ollama

> **Depends on**: T-domain-types.

## Goal

Implement the ModelProvider port for all OpenAI-API-shaped providers.
Handle SSE streaming, tool-call delta accumulation, and per-provider
auth headers and base URLs.


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

- Test: OpenRouter request includes HTTP-Referer and X-Title headers.
- Test: Ollama base URL defaults to localhost:11434.
- Test: reasoning_effort field absent for non-o-series models.
- Test: SSE stream with multiple tool-call chunks assembles correct arguments JSON.


### 2. GREEN — Implement to pass

- Single struct OpenAICompatibleModel { base_url, api_key, model, reasoning_effort, extra_headers, tool_defs }.
- Providers: OpenAI (https://api.openai.com/v1), OpenRouter (https://openrouter.ai/api/v1, adds HTTP-Referer + X-Title headers), Cerebras (https://api.cerebras.ai/v1), Ollama (http://localhost:11434/v1, 120s first-byte timeout).
- OpenAI reasoning: inject 'reasoning_effort': 'high'/'medium'/'low' in request body for o-series models.
- Streaming: accumulate tool-call argument delta strings by index.
- Tool call deserialisation: handle both {type:'function', function:{name,arguments}} shapes.
- factory function: OpenAICompatibleModel::for_provider(kind, credential_bundle, model, effort, tool_defs).


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

Update PROGRESS.md row for T07 to `[x]`.
Commit: `feat(provider-openai-compat): implement openaicompatiblemodel: openai, openrouter, cerebras, ollama`
