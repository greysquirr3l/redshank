//! `ToolDispatcher` port — tool execution interface.

use crate::domain::auth::AuthContext;
use crate::domain::errors::DomainError;
use crate::domain::session::ToolResult;
use serde_json::Value;

/// Port trait for dispatching tool calls.
pub trait ToolDispatcher: Send + Sync {
    /// Dispatch a tool call by name with the given arguments.
    /// Requires an `AuthContext` for security policy enforcement.
    fn dispatch(
        &self,
        auth: &AuthContext,
        tool_name: &str,
        arguments: Value,
    ) -> impl std::future::Future<Output = Result<ToolResult, DomainError>> + Send;
}
