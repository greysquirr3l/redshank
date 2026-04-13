//! Security domain model: `AuthContext`, `Permission`, `Role`, `SecurityPolicy`.
//!
//! Security rules are pure domain functions with zero I/O. It is structurally
//! impossible to call a repository method without providing an `AuthContext`
//! and having the policy evaluated. Default deny everywhere.
//!
//! The fail-secure invariant: every path through a domain security function
//! either calls `policy.check()` or explicitly returns
//! `Err(SecurityError::AccessDenied)`. There is no default-allow code path.

use crate::domain::credentials::CredentialGuard;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ── UserId ──────────────────────────────────────────────────

/// Unique identifier for a user or system principal.
///
/// Newtype around `Uuid`. Does NOT implement `Copy` — forces intentional passing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Uuid);

impl UserId {
    /// Create a new random user ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a user ID from an existing UUID.
    #[must_use]
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// The well-known system user ID (nil UUID).
    #[must_use]
    pub const fn system() -> Self {
        Self(Uuid::nil())
    }

    /// Access the inner UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Role ────────────────────────────────────────────────────

/// Role levels determining permission sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Full access: all 8 permissions.
    Owner,
    /// Operational access: run agents, read/write sessions and wiki, fetch data.
    Operator,
    /// Read-only: view sessions and wiki.
    Reader,
    /// Machine-to-machine: internal operations like wiki seed.
    Service,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner => write!(f, "Owner"),
            Self::Operator => write!(f, "Operator"),
            Self::Reader => write!(f, "Reader"),
            Self::Service => write!(f, "Service"),
        }
    }
}

// ── Permission ──────────────────────────────────────────────

/// Granular permissions checked by the security policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Read session data.
    ReadSession,
    /// Write/create session data.
    WriteSession,
    /// Start an agent investigation.
    RunAgent,
    /// Delete a session.
    DeleteSession,
    /// Configure API credentials.
    ConfigureCredentials,
    /// Read configuration (sources, providers, settings).
    ReadConfiguration,
    /// Update data source configuration.
    ConfigureSources,
    /// Update model provider configuration.
    ConfigureProviders,
    /// Read wiki entries.
    ReadWiki,
    /// Write wiki entries.
    WriteWiki,
    /// Execute a data fetcher.
    FetchData,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// ── SecurityError ───────────────────────────────────────────

/// Error returned when a security check fails.
///
/// All variants carry enough context for audit logging without leaking
/// credential data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityError {
    /// The user does not have the required permission.
    AccessDenied {
        /// The user ID that was denied.
        user_id: UserId,
        /// The permission that was required.
        required_permission: Permission,
    },
    /// The authentication token is invalid.
    InvalidToken,
    /// The authentication token has expired.
    ExpiredToken,
    /// The user's role is insufficient for the operation.
    InsufficientRole {
        /// The user ID.
        user_id: UserId,
        /// The role that was required.
        required_role: Role,
        /// The user's actual roles.
        actual_roles: Vec<Role>,
    },
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessDenied {
                user_id,
                required_permission,
            } => write!(
                f,
                "access denied: user {user_id} lacks {required_permission}"
            ),
            Self::InvalidToken => write!(f, "invalid authentication token"),
            Self::ExpiredToken => write!(f, "expired authentication token"),
            Self::InsufficientRole {
                user_id,
                required_role,
                actual_roles,
            } => write!(
                f,
                "insufficient role for user {user_id}: required {required_role}, has {actual_roles:?}"
            ),
        }
    }
}

impl std::error::Error for SecurityError {}

// ── AuthContext ──────────────────────────────────────────────

/// Identity and roles of the caller making a request.
///
/// Does NOT implement `Copy` or careless `Clone` — use `Arc<AuthContext>`
/// at call sites that need to share it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthContext {
    /// Unique user identity.
    pub user_id: UserId,
    /// Set of roles assigned to this user.
    pub roles: Vec<Role>,
    /// Session token (redacted in Debug/Display output).
    pub session_token: CredentialGuard<String>,
}

impl AuthContext {
    /// Create a system-level auth context with `Role::Service`.
    #[must_use]
    pub fn system() -> Self {
        Self {
            user_id: UserId::system(),
            roles: vec![Role::Service],
            session_token: CredentialGuard::new("system-internal".to_string()),
        }
    }

    /// Create an owner auth context.
    #[must_use]
    pub fn owner(user_id: UserId, token: String) -> Self {
        Self {
            user_id,
            roles: vec![Role::Owner],
            session_token: CredentialGuard::new(token),
        }
    }

    /// Check if the context has a specific role.
    #[must_use]
    pub fn has_role(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }
}

// ── SecurityPolicy trait ────────────────────────────────────

/// Trait for evaluating security policy. Pure domain — no I/O.
///
/// Object-safe: uses `&self` reference. Test code can inject mock policies.
pub trait SecurityPolicy: Send + Sync {
    /// Check whether the given auth context has the specified permission.
    ///
    /// # Errors
    ///
    /// Returns `Err(SecurityError::AccessDenied)` if the auth context does not
    /// hold a role that grants `permission`.
    fn check(&self, auth: &AuthContext, permission: Permission) -> Result<(), SecurityError>;
}

// ── StaticPolicy ────────────────────────────────────────────

/// Default static role-based permission policy.
///
/// Permission map:
/// - **Owner** → all permissions
/// - **Operator** → `RunAgent`, `ReadSession`, `WriteSession`, `ReadWiki`, `WriteWiki`, `FetchData`, `ReadConfiguration`, `ConfigureSources`, `ConfigureProviders`
/// - **Reader** → `ReadSession`, `ReadWiki`, `ReadConfiguration`
/// - **Service** → `ReadSession`, `WriteSession`, `ReadWiki`, `WriteWiki`, `FetchData`
#[derive(Debug, Clone)]
pub struct StaticPolicy;

impl StaticPolicy {
    /// Returns `true` if the given role grants the given permission.
    const fn role_grants(role: Role, permission: Permission) -> bool {
        match role {
            Role::Owner => true,
            Role::Operator => matches!(
                permission,
                Permission::RunAgent
                    | Permission::ReadSession
                    | Permission::WriteSession
                    | Permission::ReadWiki
                    | Permission::WriteWiki
                    | Permission::FetchData
                    | Permission::ReadConfiguration
                    | Permission::ConfigureSources
                    | Permission::ConfigureProviders
            ),
            Role::Reader => matches!(
                permission,
                Permission::ReadSession | Permission::ReadWiki | Permission::ReadConfiguration
            ),
            Role::Service => matches!(
                permission,
                Permission::ReadSession
                    | Permission::WriteSession
                    | Permission::ReadWiki
                    | Permission::WriteWiki
                    | Permission::FetchData
            ),
        }
    }
}

impl SecurityPolicy for StaticPolicy {
    fn check(&self, auth: &AuthContext, permission: Permission) -> Result<(), SecurityError> {
        for role in &auth.roles {
            if Self::role_grants(*role, permission) {
                return Ok(());
            }
        }
        Err(SecurityError::AccessDenied {
            user_id: auth.user_id.clone(),
            required_permission: permission,
        })
    }
}

// ── Pure domain security check functions ────────────────────

/// Check permission to read session data.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_read_session(
    ctx: &AuthContext,
    policy: &dyn SecurityPolicy,
) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::ReadSession)
}

/// Check permission to delete a session.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_delete_session(
    ctx: &AuthContext,
    policy: &dyn SecurityPolicy,
) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::DeleteSession)
}

/// Check permission to write (create/update) session data.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_write_session(
    ctx: &AuthContext,
    policy: &dyn SecurityPolicy,
) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::WriteSession)
}

/// Check permission to run an agent investigation.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_run_agent(ctx: &AuthContext, policy: &dyn SecurityPolicy) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::RunAgent)
}

/// Check permission to write wiki entries.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_write_wiki(ctx: &AuthContext, policy: &dyn SecurityPolicy) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::WriteWiki)
}

/// Check permission to read wiki entries.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_read_wiki(ctx: &AuthContext, policy: &dyn SecurityPolicy) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::ReadWiki)
}

/// Check permission to fetch external data.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_fetch_data(ctx: &AuthContext, policy: &dyn SecurityPolicy) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::FetchData)
}

/// Check permission to configure credentials.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_configure_credentials(
    ctx: &AuthContext,
    policy: &dyn SecurityPolicy,
) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::ConfigureCredentials)
}

/// Check permission to read configuration (providers, sources, settings).
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_read_configuration(
    ctx: &AuthContext,
    policy: &dyn SecurityPolicy,
) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::ReadConfiguration)
}

/// Check permission to configure data sources.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_configure_sources(
    ctx: &AuthContext,
    policy: &dyn SecurityPolicy,
) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::ConfigureSources)
}

/// Check permission to configure model providers.
///
/// # Errors
///
/// Returns `Err(SecurityError::AccessDenied)` if denied by the policy.
pub fn can_configure_providers(
    ctx: &AuthContext,
    policy: &dyn SecurityPolicy,
) -> Result<(), SecurityError> {
    policy.check(ctx, Permission::ConfigureProviders)
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    fn owner_ctx() -> AuthContext {
        AuthContext::owner(UserId::new(), "test-token".into())
    }

    fn reader_ctx() -> AuthContext {
        AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Reader],
            session_token: CredentialGuard::new("reader-token".into()),
        }
    }

    fn operator_ctx() -> AuthContext {
        AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Operator],
            session_token: CredentialGuard::new("operator-token".into()),
        }
    }

    fn service_ctx() -> AuthContext {
        AuthContext::system()
    }

    #[test]
    fn static_policy_grants_owner_all_8_permissions() {
        let policy = StaticPolicy;
        let ctx = owner_ctx();
        let all_perms = [
            Permission::ReadSession,
            Permission::WriteSession,
            Permission::RunAgent,
            Permission::DeleteSession,
            Permission::ConfigureCredentials,
            Permission::ReadWiki,
            Permission::WriteWiki,
            Permission::FetchData,
        ];
        for perm in &all_perms {
            assert!(
                policy.check(&ctx, *perm).is_ok(),
                "Owner should have {perm:?}"
            );
        }
    }

    #[test]
    fn static_policy_denies_reader_delete_session() {
        let policy = StaticPolicy;
        let ctx = reader_ctx();
        assert!(policy.check(&ctx, Permission::DeleteSession).is_err());
    }

    #[test]
    fn static_policy_denies_operator_configure_credentials() {
        let policy = StaticPolicy;
        let ctx = operator_ctx();
        assert!(
            policy
                .check(&ctx, Permission::ConfigureCredentials)
                .is_err()
        );
    }

    #[test]
    fn can_read_session_ok_with_owner() {
        let policy = StaticPolicy;
        assert!(can_read_session(&owner_ctx(), &policy).is_ok());
    }

    #[test]
    fn can_delete_session_denied_for_reader() {
        let policy = StaticPolicy;
        let result = can_delete_session(&reader_ctx(), &policy);
        assert!(result.is_err());
        if let Err(SecurityError::AccessDenied {
            required_permission,
            ..
        }) = result
        {
            assert_eq!(required_permission, Permission::DeleteSession);
        } else {
            panic!("expected AccessDenied");
        }
    }

    #[test]
    fn system_context_has_service_role() {
        let ctx = AuthContext::system();
        assert_eq!(ctx.user_id, UserId::system());
        assert!(ctx.has_role(Role::Service));
    }

    #[test]
    fn access_denied_carries_correct_fields() {
        let policy = StaticPolicy;
        let ctx = reader_ctx();
        let user_id = ctx.user_id.clone();
        let result = policy.check(&ctx, Permission::RunAgent);
        match result {
            Err(SecurityError::AccessDenied {
                user_id: denied_id,
                required_permission,
            }) => {
                assert_eq!(denied_id, user_id);
                assert_eq!(required_permission, Permission::RunAgent);
            }
            _ => panic!("expected AccessDenied"),
        }
    }

    #[test]
    fn security_error_display_contains_no_credentials() {
        let err = SecurityError::AccessDenied {
            user_id: UserId::system(),
            required_permission: Permission::RunAgent,
        };
        let display = format!("{err}");
        assert!(!display.contains("token"));
        assert!(!display.contains("secret"));
        assert!(display.contains("access denied"));

        let err2 = SecurityError::InvalidToken;
        assert!(!format!("{err2}").contains("secret"));

        let err3 = SecurityError::ExpiredToken;
        assert!(!format!("{err3}").contains("secret"));

        let err4 = SecurityError::InsufficientRole {
            user_id: UserId::system(),
            required_role: Role::Owner,
            actual_roles: vec![Role::Reader],
        };
        let display4 = format!("{err4}");
        assert!(!display4.contains("token"));
        assert!(display4.contains("insufficient role"));
    }

    #[test]
    fn service_role_can_read_and_write_but_not_delete() {
        let policy = StaticPolicy;
        let ctx = service_ctx();
        assert!(policy.check(&ctx, Permission::ReadSession).is_ok());
        assert!(policy.check(&ctx, Permission::WriteSession).is_ok());
        assert!(policy.check(&ctx, Permission::ReadWiki).is_ok());
        assert!(policy.check(&ctx, Permission::WriteWiki).is_ok());
        assert!(policy.check(&ctx, Permission::FetchData).is_ok());
        assert!(policy.check(&ctx, Permission::DeleteSession).is_err());
        assert!(
            policy
                .check(&ctx, Permission::ConfigureCredentials)
                .is_err()
        );
        assert!(policy.check(&ctx, Permission::RunAgent).is_err());
    }

    #[test]
    fn user_id_display() {
        let id = UserId::system();
        assert_eq!(format!("{id}"), Uuid::nil().to_string());
    }
}
