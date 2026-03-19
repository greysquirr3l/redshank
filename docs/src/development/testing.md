# Testing

## Running tests

```bash
# Full workspace
cargo test --workspace

# Single crate
cargo test -p redshank-core
cargo test -p redshank-fetchers
```

## Test organisation

| Module | What's tested |
|--------|---------------|
| `domain/` | Pure unit tests — no I/O, no async |
| `application/commands/` | Command handlers with in-memory stores |
| `application/services/` | Engine loop with scripted model |
| `adapters/persistence/` | SQLite store with temp-dir database |
| `adapters/providers/` | Provider parsers with fixture responses |
| `adapters/tools/` | Workspace tools with temp-dir workspace |
| `redshank-fetchers/` | Fetcher HTTP logic with `wiremock` |
| `redshank-core/tests/` | Full-stack integration test |

## TDD policy

Write a failing test before implementation code. Every public function needs at least one test. All tests must pass before a task is marked complete.

## Integration test

`redshank-core/tests/integration.rs` runs a full agent loop using a scripted `MockModelProvider` that returns deterministic tool calls and a final answer. No network access required.
