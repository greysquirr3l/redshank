# T06 — AnthropicModel: native Messages API with SSE streaming and thinking budgets

> **Depends on**: T-domain-types.

## Goal

Implement the ModelProvider port for Anthropic. Handle Claude models including
adaptive thinking (Opus 4.6+) and manual thinking budgets. Parse SSE events for
streaming content and tool-call delta accumulation.


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

- Test: SSE event stream assembled from fixture bytes produces correct ModelTurn.
- Test: tool-call JSON fragment accumulation across multiple deltas.
- Test: thinking budget is absent in payload when reasoning_effort is None.
- Test: API key never appears in error messages or panic output.


### 2. GREEN — Implement to pass

- Use reqwest with the rustls-tls feature; no native-tls anywhere.
- POST to https://api.anthropic.com/v1/messages with anthropic-version: 2023-06-01 header.
- Streaming: parse event: / data: SSE lines; accumulate content_block_delta text and tool-call input JSON fragments.
- Tool serialisation: convert TOOL_DEFINITIONS to Anthropic's {name, description, input_schema} format.
- Reasoning: for claude-opus-4-6 use 'thinking': {'type':'enabled','budget_tokens': N}; for other models inject <thinking> prefix in system prompt when effort != None.
- count_tokens() calls POST /v1/messages/count_tokens (dry-run, no streaming).
- AnthropicModel::new(api_key, model, reasoning_effort, tool_defs).
- Map stop_reason strings: 'end_turn' → StopReason::EndTurn, 'tool_use' → StopReason::ToolUse, 'max_tokens' → StopReason::MaxTokens.


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

Update PROGRESS.md row for T06 to `[x]`.
Commit: `feat(provider-anthropic): implement anthropicmodel: native messages api with sse streaming and thinking budgets`
