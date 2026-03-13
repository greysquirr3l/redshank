//! Security domain model: `AuthContext`, `Permission`, `Role`, `SecurityPolicy`.
//!
//! Security rules are pure domain functions with zero I/O. It is structurally
//! impossible to call a repository method without providing an `AuthContext`
//! and having the policy evaluated. Default deny everywhere.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Identity and role of the caller making a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    /// User or system identity.
    pub principal: String,
    /// Role determining permission set.
    pub role: Role,
}

impl AuthContext {
    /// Create a system-level auth context (full permissions).
    pub fn system() -> Self {
        Self {
            principal: "system".to_string(),
            role: Role::System,
        }
    }

    /// Create a guest auth context (minimal permissions).
    pub fn guest() -> Self {
        Self {
            principal: "guest".to_string(),
            role: Role::Guest,
        }
    }
}

/// Role levels with increasing privilege.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Role {
    /// No authenticated identity. Read-only public access.
    Guest,
    /// Authenticated user. Can run investigations and read own sessions.
    User,
    /// Elevated user. Can access credentials and run fetchers.
    Admin,
    /// Internal system identity. Full access.
    System,
}

/// Granular permissions checked by the security policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Read session data.
    ReadSession,
    /// Write/create session data.
    WriteSession,
    /// Start an agent investigation.
    RunAgent,
    /// Access credential store.
    AccessCredentials,
    /// Execute a data fetcher.
    RunFetcher,
    /// Administrative operations.
    AdministerSystem,
}

/// Error returned when a security check fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityError {
    /// The permission that was denied.
    pub permission: Permission,
    /// The principal that was denied.
    pub principal: String,
    /// Human-readable context for audit logging.
    pub context: String,
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "access denied: principal '{}' lacks {:?} ({})",
            self.principal, self.permission, self.context
        )
    }
}

impl std::error::Error for SecurityError {}

/// Trait for evaluating security policy. Pure domain — no I/O.
pub trait SecurityPolicy: Send + Sync {
    /// Check whether the given auth context has the specified permission.
    /// Returns `Ok(())` if granted, `Err(SecurityError)` if denied.
    fn check(
        &self,
        auth: &AuthContext,
        permission: Permission,
    ) -> Result<(), SecurityError>;
}

/// Default static policy: role-based permission mapping.
#[derive(Debug, Clone)]
pub struct StaticPolicy;

impl SecurityPolicy for StaticPolicy {
    fn check(
        &self,
        auth: &AuthContext,
        permission: Permission,
    ) -> Result<(), SecurityError> {
        let granted = match permission {
            Permission::ReadSession => auth.role >= Role::Guest,
            Permission::WriteSession => auth.role >= Role::User,
            Permission::RunAgent => auth.role >= Role::User,
            Permission::AccessCredentials => auth.role >= Role::Admin,
            Permission::RunFetcher => auth.role >= Role::User,
            Permission::AdministerSystem => auth.role >= Role::System,
        };

        if granted {
            Ok(())
        } else {
            Err(SecurityError {
                permission,
                principal: auth.principal.clone(),
                context: format!("role {:?} insufficient for {:?}", auth.role, permission),
            })
        }
    }
}
