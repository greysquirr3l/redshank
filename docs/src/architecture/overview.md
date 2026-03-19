# Architecture Overview

Redshank is structured as a Cargo workspace of four crates:

| Crate | Role |
|-------|------|
| `redshank-core` | Domain model, ports, application layer, adapters |
| `redshank-cli` | `clap` binary entry point |
| `redshank-tui` | `ratatui` TUI event loop and renderer |
| `redshank-fetchers` | 34 data fetcher implementations |

## Internal layout (`redshank-core`)

The internal layout follows hexagonal DDD with explicit CQRS:

```
src/
  domain/          # Pure types — zero I/O, zero async
  ports/           # Trait interfaces (inbound + outbound)
  application/
    commands/      # Mutating CQRS handlers + IdempotencyKey
    queries/       # Read-only CQRS handlers
    services/      # Orchestration (agent engine, condensation)
  adapters/
    providers/     # LLM provider impls (Anthropic, OpenAI-compat)
    tools/         # WorkspaceTools (filesystem, shell, web, patching)
    persistence/   # SQLite session store
```

## Dependency rule

No domain type may reference an adapter or application type. The compiler enforces this: `redshank-core/Cargo.toml` has zero I/O crates (tokio, reqwest, sqlx) as non-optional direct dependencies.

## Further reading

- [Hexagonal DDD](./hexagonal-ddd.md)
- [CQRS & Domain Events](./cqrs-events.md)
- [Security Model](./security.md)
- [Agent Engine](./engine.md)
- [Wiki Graph](./wiki-graph.md)
