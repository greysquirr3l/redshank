# Agent Engine

The core agent loop lives in `redshank-core/src/application/services/engine.rs`.

## Loop structure

```
RunInvestigationCommand
  └─► RunInvestigationHandler
        └─► RLMEngine::solve(objective)
              └─► solve_recursive(objective, depth=0)
                    ├─ model.complete(messages, tools)
                    ├─ process_tool_calls(...)
                    │    ├─ tool_dispatcher.dispatch(tool_name, args)
                    │    └─ solve_recursive(subtask, depth+1)  [for delegate_task]
                    └─ condense_tool_outputs() [if token budget > 75%]
```

## Key parameters

| Field | Type | Description |
|-------|------|-------------|
| `max_steps` | `u32` | Step budget per invocation (default: 50) |
| `max_depth` | `u32` | Maximum subtask recursion depth (default: 3) |
| `workspace` | `PathBuf` | Working directory for file and shell tools |

## Context condensation

When token usage exceeds 75% of the model's context window, `condense_tool_outputs` truncates older tool results in place, keeping the original objective and recent messages intact.

## Acceptance criteria

An optional judge model evaluates the final answer against the original objective. The judge uses a lightweight model (configurable) to return `pass` or `fail` with a rationale. On `fail`, the engine attempts one more solve pass with the judge's feedback injected.

## Cancellation

The engine checks a `CancellationToken` at the start of each step (when built with the `runtime` feature). Cancellation is clean — the engine returns the best answer accumulated so far.
