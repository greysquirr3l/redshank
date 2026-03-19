# Hexagonal DDD

Redshank uses a DDD-Lite hexagonal architecture. The key invariant is the **dependency rule**: inner rings know nothing about outer rings.

```
          ┌─────────────────────────────────────┐
          │           Adapters (outer)           │
          │  ┌───────────────────────────────┐  │
          │  │      Application layer        │  │
          │  │  ┌─────────────────────────┐  │  │
          │  │  │    Domain (inner)        │  │  │
          │  │  │  Types, events, rules    │  │  │
          │  │  └─────────────────────────┘  │  │
          │  │  Commands / Queries / Services │  │
          │  └───────────────────────────────┘  │
          │  Providers / Tools / Persistence     │
          └─────────────────────────────────────┘
```

## Domain layer (`src/domain/`)

Pure Rust structs and enums. No `async`, no `tokio`, no I/O of any kind.

- `agent.rs` — `AgentSession` aggregate root, `AgentConfig`, `ProviderKind`
- `auth.rs` — `AuthContext`, `Role`, `Permission`, `SecurityPolicy`
- `credentials.rs` — `CredentialBundle`, resolution order
- `errors.rs` — `DomainError` enum (all `thiserror`)
- `events.rs` — `DomainEvent` variants
- `session.rs` — session value objects
- `settings.rs` — `PersistentSettings`
- `wiki.rs` — `WikiEntry`, `WikiEntryId`

## Ports layer (`src/ports/`)

Trait interfaces that the domain and application layers call. Adapters implement these traits; the domain never sees the implementations.

- `ModelProvider` — LLM completions with streaming
- `SessionStore` — session persistence (CQRS-aware)
- `WikiStore` — wiki entry persistence
- `ToolDispatcher` — tool invocation
- `ReplayLog` — JSONL delta-encoded call log
- `FetcherPort` — data fetcher abstraction
