# T05 — Security domain model: AuthContext, Permission, SecurityPolicy (fail-secure, DDD-Lite)

> **Depends on**: T-domain-types.

## Goal

Implement the security model in redshank-core/src/domain/auth.rs following the
security-first repository design principle: security rules are pure domain functions
with zero I/O — it is structurally impossible to call a repository method without
providing an AuthContext and having the policy evaluated. Default deny everywhere.
Based on docs/dev/security_first_repository_design.md.


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

- Test: StaticPolicy grants Owner all 8 permissions.
- Test: StaticPolicy denies Reader permission DeleteSession.
- Test: StaticPolicy denies Operator permission ConfigureCredentials.
- Test: can_read_session returns Ok with Owner context.
- Test: can_delete_session returns AccessDenied for Reader context.
- Test: AuthContext::system() has Role::Service.
- Test: SecurityError::AccessDenied carries correct user_id and required_permission.
- Test: all SecurityError Display impls contain no credential material.


### 2. GREEN — Implement to pass

- UserId: newtype around Uuid. Display shows the UUID string. Debug shows UserId("...") for audit log clarity. Does NOT implement Copy — forces intentional passing.
- Role enum: Owner (full access), Operator (run + read + write sessions, wiki, fetch), Reader (read-only sessions + wiki), Service (machine-to-machine for internal ops like wiki seed).
- Permission enum: ReadSession, WriteSession, RunAgent, DeleteSession, ConfigureCredentials, ReadWiki, WriteWiki, FetchData.
- SecurityPolicy trait (object-safe): fn check(&self, ctx: &AuthContext, permission: Permission) -> Result<(), SecurityError>. Default impl StaticPolicy denies ALL permissions — any grant must be explicit.
- StaticPolicy permission map: Owner → all 8 permissions; Operator → RunAgent, ReadSession, WriteSession, ReadWiki, WriteWiki, FetchData; Reader → ReadSession, ReadWiki; Service → ReadSession, WriteSession, ReadWiki, WriteWiki, FetchData.
- SecurityError enum: AccessDenied { user_id: UserId, required_permission: Permission }, InvalidToken, ExpiredToken, InsufficientRole { user_id: UserId, required_role: Role, actual_roles: Vec<Role> }. All variants carry enough context for audit logging without leaking credential data.
- AuthContext { user_id: UserId, roles: Vec<Role>, session_token: CredentialGuard<String> }. Does NOT implement Copy or Clone carelessly — use Arc<AuthContext> at call sites that need to share it. AuthContext::system() -> AuthContext returns Role::Service for internal ops.
- Domain security functions in auth.rs (pure — no I/O, no async, no side effects — takes a &dyn SecurityPolicy so the test can inject a mock): fn can_read_session(ctx, policy) -> Result; fn can_delete_session(ctx, policy) -> Result; fn can_run_agent(ctx, policy) -> Result; fn can_write_wiki(ctx, policy) -> Result; fn can_fetch_data(ctx, policy) -> Result; fn can_configure_credentials(ctx, policy) -> Result.
- Fail-secure invariant: every path through a domain security function either calls policy.check() or explicitly returns Err(SecurityError::AccessDenied). There is no default-allow code path.


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

Update PROGRESS.md row for T05 to `[x]`.
Commit: `feat(security-model): implement security domain model: authcontext, permission, securitypolicy (fail-secure, ddd-lite)`
