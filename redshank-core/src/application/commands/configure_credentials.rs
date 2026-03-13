//! `ConfigureCredentials` command.

use serde::{Deserialize, Serialize};
use super::run_investigation::IdempotencyKey;

/// Command to update credential configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigureCredentialsCommand {
    /// Idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Key name (e.g. "anthropic_api_key").
    pub key_name: String,
    /// New value.
    pub value: String,
}

// TODO(T03): Implement ConfigureCredentialsHandler
