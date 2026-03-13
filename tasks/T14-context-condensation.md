# T14 — Context condensation and turn summaries

> **Depends on**: T-provider-builder, T-domain-types.

## Goal

Implement context-window tracking and turn-summary injection:
when token usage exceeds 75% of the model's context window, inject
a condensation turn that summarises past reasoning.


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

- Test: should_condense() false at 74% usage, true at 76%.
- Test: condense() call produces a Conversation with fewer turns than the original.
- Test: TurnSummary appended to session state after each completed step.


### 2. GREEN — Implement to pass

- ContextTracker { window_size: u32, used_tokens: u32 }.
- should_condense() → bool when used_tokens / window_size > 0.75.
- condense(conversation, judge_model) → Conversation: calls the judge model with 'Summarise the investigation so far in 400 words.' and replaces old turns with a single <condensation> system turn.
- TurnSummary items persisted to session state after each step.
- Model context window sizes: Claude models 200k, GPT-4.1 128k, o4-mini 128k, Cerebras models 128k, Ollama 8k default (overridable).


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

Update PROGRESS.md row for T14 to `[x]`.
Commit: `feat(context-condensation): implement context condensation and turn summaries`
