# CQRS & Domain Events

## Command handlers

Every mutating operation is a `Command` struct carrying an `IdempotencyKey` (newtype `Uuid` v4), handled by a `CommandHandler` in `application/commands/`.

```rust
pub struct RunInvestigationCommand {
    pub idempotency_key: IdempotencyKey,
    pub objective: String,
    pub config: AgentConfig,
    pub auth: AuthContext,
}
```

Handlers check the `idempotency_keys` table before executing. Duplicate commands return the cached result without re-running.

## Query handlers

Every read operation is a `Query` struct handled by a `QueryHandler` in `application/queries/`. Queries never mutate state.

## Domain events

Every significant state transition emits a typed `DomainEvent` variant:

| Variant | Trigger |
|---------|---------|
| `SessionCreated` | New session initialised |
| `AgentStarted` | Investigation loop begins |
| `ToolCalled` | A tool is dispatched |
| `AgentCompleted` | Loop returns a final answer |
| `WikiEntryWritten` | A wiki page is created or updated |

Events are immutable value types. Aggregate methods append them to a `pending_events: Vec<DomainEvent>`; the session store persists them via `append_event`.
