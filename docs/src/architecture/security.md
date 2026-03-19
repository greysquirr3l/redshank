# Security Model

## Default deny

Every port method that accesses or mutates keyed data accepts `auth: &AuthContext` and calls `SecurityPolicy::check` before any data access. The default is `Err(SecurityError::AccessDenied)` unless the policy explicitly grants the required `Permission`.

## Roles and permissions

| Role | Permissions |
|------|-------------|
| `Reader` | Read sessions, read wiki |
| `Operator` | All Reader permissions + run agent, write sessions, write wiki, call non-destructive tools |
| `Admin` | All Operator permissions + manage credentials, delete sessions |

## AuthContext

```rust
pub struct AuthContext {
    pub user_id: UserId,
    pub role: Role,
}
```

`AuthContext` is created at the CLI/TUI boundary and propagated through every command and query handler. It never crosses crate boundaries as a raw struct — it's always validated at the port layer.

## Credential security

- Credentials are stored in `~/.redshank/credentials` with `chmod 600`.
- Keys never appear in log output at any level (enforced by `Debug` impls that redact sensitive fields).
- No credential is ever written to disk with broader permissions than `0o600`.

## Security rules location

All policy logic lives in `redshank-core/src/domain/auth.rs` as pure functions — no I/O, no async. This makes the security model independently testable without any infrastructure.
