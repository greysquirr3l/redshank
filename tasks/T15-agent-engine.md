# T15 — RLMEngine: recursive tool-calling agent loop (application/services/ + CQRS command handler)

> **Depends on**: T-workspace-tools, T-provider-builder, T-replay-logger, T-context-condensation, T-patching, T-security-model.

## Goal

Implement the core agent loop in redshank-core/src/application/services/engine.rs,
mirroring agent/engine.py. The CQRS entry point is RunInvestigationHandler in
application/commands/run_investigation.rs: it validates the command, checks idempotency
(mark as in-flight, return cached result on duplicate), enforces AuthContext via
can_run_agent(), then delegates to RLMEngine. Features: step budget, depth control,
subtask recursion (child RunInvestigationCommands), runtime policy enforcement,
acceptance-criteria judge, cancel() via CancellationToken.


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

- Test (scripted model): write_file then read_file then end_turn produces expected final text.
- Test: subtask recursion to depth 2 produces nested JSONL log entries.
- Test: depth limit returns error ToolResult at max_depth.
- Test: step budget exhaustion returns BudgetExhausted error.
- Test: identical run_shell blocked after second repetition.
- Test: cancel() during model.complete() returns CancelledError within 500ms.
- Test: parallel subtasks run concurrently (mock model sleeps 100ms; two subtasks complete in <200ms total).
- Test: acceptance judge retry on first fail passes on second attempt.


### 2. GREEN — Implement to pass

- RunInvestigationHandler (application/commands/run_investigation.rs): (1) check_idempotency_key → return cached if hit; (2) can_run_agent(auth, policy) → return Err if denied; (3) mark key in-flight; (4) build RLMEngine; (5) call solve(); (6) store AgentCompleted event via session_store.append_event(); (7) mark_idempotency_key(key, result). Returns Err(DomainError) on auth failure or engine error.
- RLMEngine (application/services/engine.rs): { config: AgentConfig, model: Arc<dyn ModelProvider>, tools: Arc<dyn ToolDispatcher>, judge: Arc<dyn ModelProvider>, replay_log: Arc<dyn ReplayLog>, auth: AuthContext, policy: Arc<dyn SecurityPolicy> }.
- solve(objective, conversation_seed) → Result<String>: calls _solve_recursive(depth=0).
- _solve_recursive(depth, objective, conversation) → Result<String>: loop up to config.max_steps per call.
- Each iteration: model.complete(conversation) → ModelTurn; log call; dispatch each ToolCall; append ToolResults; check StopReason::EndTurn.
- Subtask: when tool name == 'subtask', create child RLMEngine with depth+1; call _solve_recursive; feed result back as ToolResult.
- Depth guard: if depth >= config.max_depth, return an error ToolResult instead of recursing.
- Runtime policy: block identical run_shell command strings seen >2× at the same depth; block write_file to an unread path (enforced again here as double-check).
- Acceptance judge: after subtask returns, call judge.complete(acceptance_prompt(subtask_goal, acceptance_criteria, result)) → pass/fail; retry once on fail.
- Context condensation: call ContextTracker after each model.complete(); condense if needed.
- CancellationToken (tokio_util::sync::CancellationToken): check at start of each loop iteration; return CancelledError if cancelled.
- Parallel subtasks: when multiple subtask ToolCalls appear in the same ModelTurn, spawn them as tokio::task::spawn() and join_all(); respect parallel write groups.
- ExternalContext { observations: Vec<String> }: accumulate cross-turn notes; prepend to system prompt on subsequent turns.


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

Update PROGRESS.md row for T15 to `[x]`.
Commit: `feat(agent-engine): implement rlmengine: recursive tool-calling agent loop (application/services/ + cqrs command handler)`
