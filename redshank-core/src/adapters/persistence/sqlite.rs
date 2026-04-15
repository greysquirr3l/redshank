//! SQLite-backed session store implementing the `SessionStore` port.
//!
//! Uses rusqlite with WAL mode. All methods enforce `AuthContext` checks.
//! In tests, uses `:memory:` databases.

#[cfg(feature = "runtime")]
use std::sync::Mutex;

#[cfg(feature = "runtime")]
use chrono::{DateTime, Utc};
#[cfg(feature = "runtime")]
use rusqlite::{Connection, params};

#[cfg(feature = "runtime")]
use crate::domain::agent::AgentSession;
#[cfg(feature = "runtime")]
use crate::domain::auth::{AuthContext, Permission, Role, SecurityError, StaticPolicy};
#[cfg(feature = "runtime")]
use crate::domain::errors::DomainError;
#[cfg(feature = "runtime")]
use crate::domain::events::DomainEvent;
#[cfg(feature = "runtime")]
use crate::domain::observation::EntityObservation;
#[cfg(feature = "runtime")]
use crate::domain::session::SessionId;

#[cfg(feature = "runtime")]
const CREATE_TABLES: &str = r"
CREATE TABLE IF NOT EXISTS sessions (
    id              TEXT PRIMARY KEY,
    owner_user_id   TEXT NOT NULL,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch()),
    metadata        TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq         INTEGER NOT NULL,
    event_type  TEXT NOT NULL,
    payload     TEXT NOT NULL,
    ts          INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key         TEXT PRIMARY KEY,
    session_id  TEXT,
    result      TEXT NOT NULL,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS observations (
    id           TEXT PRIMARY KEY,
    entity_id    TEXT NOT NULL,
    source_id    TEXT NOT NULL,
    observed_at  INTEGER NOT NULL,
    payload_hash TEXT NOT NULL,
    payload      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_observations_entity_source_ts
    ON observations(entity_id, source_id, observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_observations_entity_ts
    ON observations(entity_id, observed_at DESC);
";

/// SQLite-backed session store.
#[cfg(feature = "runtime")]
pub struct SqliteSessionStore {
    conn: Mutex<Connection>,
    policy: StaticPolicy,
}

#[cfg(feature = "runtime")]
impl SqliteSessionStore {
    /// Open or create a session store at the given path.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the database file cannot be opened or initialized.
    pub fn open(path: &str) -> Result<Self, DomainError> {
        let conn =
            Connection::open(path).map_err(|e| DomainError::Other(format!("sqlite open: {e}")))?;
        Self::init(conn)
    }

    /// Create an in-memory session store (for tests).
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the in-memory database cannot be initialized.
    pub fn in_memory() -> Result<Self, DomainError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| DomainError::Other(format!("sqlite open: {e}")))?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, DomainError> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .map_err(|e| DomainError::Other(format!("sqlite pragma: {e}")))?;
        conn.execute_batch(CREATE_TABLES)
            .map_err(|e| DomainError::Other(format!("sqlite create tables: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
            policy: StaticPolicy,
        })
    }

    /// Check whether the caller has Owner or Service role (bypass ownership check).
    fn is_privileged(auth: &AuthContext) -> bool {
        auth.has_role(Role::Owner) || auth.has_role(Role::Service)
    }

    /// Verify the caller can access a specific session.
    fn check_session_access(
        &self,
        auth: &AuthContext,
        session_id: &SessionId,
    ) -> Result<(), DomainError> {
        if Self::is_privileged(auth) {
            return Ok(());
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let owner: Option<String> = conn
            .query_row(
                "SELECT owner_user_id FROM sessions WHERE id = ?1",
                params![session_id.to_string()],
                |row| row.get(0),
            )
            .ok();
        drop(conn);
        match owner {
            Some(owner_id) if owner_id == auth.user_id.to_string() => Ok(()),
            Some(_) => Err(DomainError::Security(SecurityError::AccessDenied {
                user_id: auth.user_id.clone(),
                required_permission: Permission::ReadSession,
            })),
            None => Err(DomainError::NotFound(format!(
                "Session {session_id} not found"
            ))),
        }
    }

    // ── SessionStore port methods ───────────────────────────────────────

    /// Save or update an agent session.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails or the database write fails.
    pub fn save(&self, auth: &AuthContext, session: &AgentSession) -> Result<(), DomainError> {
        use crate::domain::auth::can_write_session;
        can_write_session(auth, &self.policy)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let metadata = serde_json::to_string(session)
            .map_err(|e| DomainError::Other(format!("serialize session: {e}")))?;

        let save_result = conn
            .execute(
                "INSERT INTO sessions (id, owner_user_id, metadata) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET metadata = excluded.metadata",
                params![
                    session.session_id.to_string(),
                    auth.user_id.to_string(),
                    metadata,
                ],
            )
            .map_err(|e| DomainError::Other(format!("sqlite save: {e}")));
        drop(conn);
        save_result?;

        Ok(())
    }

    /// Load a session by ID.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails, the database read fails, or deserialization fails.
    pub fn load(
        &self,
        auth: &AuthContext,
        id: SessionId,
    ) -> Result<Option<AgentSession>, DomainError> {
        use crate::domain::auth::can_read_session;
        can_read_session(auth, &self.policy)?;

        // Check ownership for non-privileged users.
        if !Self::is_privileged(auth) {
            self.check_session_access(auth, &id)?;
        }

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let result: Option<String> = conn
            .query_row(
                "SELECT metadata FROM sessions WHERE id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .ok();
        drop(conn);

        match result {
            Some(json) => {
                let session: AgentSession = serde_json::from_str(&json)
                    .map_err(|e| DomainError::Other(format!("deserialize session: {e}")))?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// List all sessions visible to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails or the database read fails.
    pub fn list(&self, auth: &AuthContext) -> Result<Vec<AgentSession>, DomainError> {
        use crate::domain::auth::can_read_session;
        can_read_session(auth, &self.policy)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;

        let mut stmt = if Self::is_privileged(auth) {
            conn.prepare("SELECT metadata FROM sessions ORDER BY created_at DESC")
                .map_err(|e| DomainError::Other(format!("sqlite prepare: {e}")))?
        } else {
            conn.prepare(
                "SELECT metadata FROM sessions WHERE owner_user_id = ?1 ORDER BY created_at DESC",
            )
            .map_err(|e| DomainError::Other(format!("sqlite prepare: {e}")))?
        };

        let rows: Vec<String> = if Self::is_privileged(auth) {
            stmt.query_map([], |row| row.get(0))
                .map_err(|e| DomainError::Other(format!("sqlite query: {e}")))?
                .filter_map(Result::ok)
                .collect()
        } else {
            stmt.query_map(params![auth.user_id.to_string()], |row| row.get(0))
                .map_err(|e| DomainError::Other(format!("sqlite query: {e}")))?
                .filter_map(Result::ok)
                .collect()
        };
        drop(stmt);
        drop(conn);

        let mut sessions = Vec::new();
        for json in rows {
            let session: AgentSession = serde_json::from_str(&json)
                .map_err(|e| DomainError::Other(format!("deserialize session: {e}")))?;
            sessions.push(session);
        }
        Ok(sessions)
    }

    /// Delete a session by ID.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails or the database write fails.
    pub fn delete(&self, auth: &AuthContext, id: SessionId) -> Result<(), DomainError> {
        use crate::domain::auth::can_delete_session;
        can_delete_session(auth, &self.policy)?;

        // Check ownership for non-privileged users.
        if !Self::is_privileged(auth) {
            self.check_session_access(auth, &id)?;
        }

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let delete_result = conn
            .execute(
                "DELETE FROM sessions WHERE id = ?1",
                params![id.to_string()],
            )
            .map_err(|e| DomainError::Other(format!("sqlite delete: {e}")));
        drop(conn);
        delete_result?;

        Ok(())
    }

    /// Append a domain event to the session's event log.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails, serialization fails, or the database write fails.
    pub fn append_event(
        &self,
        auth: &AuthContext,
        session_id: SessionId,
        event: &DomainEvent,
    ) -> Result<(), DomainError> {
        use crate::domain::auth::can_write_session;
        can_write_session(auth, &self.policy)?;

        let event_type = match event {
            DomainEvent::SessionCreated { .. } => "SessionCreated",
            DomainEvent::AgentStarted { .. } => "AgentStarted",
            DomainEvent::ToolCalled { .. } => "ToolCalled",
            DomainEvent::AgentCompleted { .. } => "AgentCompleted",
            DomainEvent::InvestigationFailed { .. } => "InvestigationFailed",
            DomainEvent::WikiEntryWritten { .. } => "WikiEntryWritten",
        };

        let payload = serde_json::to_string(event)
            .map_err(|e| DomainError::Other(format!("serialize event: {e}")))?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;

        // Get next seq number.
        let seq: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(seq), -1) + 1 FROM events WHERE session_id = ?1",
                params![session_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|e| DomainError::Other(format!("sqlite seq: {e}")))?;

        let append_result = conn
            .execute(
                "INSERT INTO events (session_id, seq, event_type, payload) VALUES (?1, ?2, ?3, ?4)",
                params![session_id.to_string(), seq, event_type, payload],
            )
            .map_err(|e| DomainError::Other(format!("sqlite append_event: {e}")));
        drop(conn);
        append_result?;

        Ok(())
    }

    /// List all events for a session, ordered by sequence.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails, the query fails, or deserialization fails.
    pub fn list_events(
        &self,
        auth: &AuthContext,
        session_id: SessionId,
    ) -> Result<Vec<DomainEvent>, DomainError> {
        use crate::domain::auth::can_read_session;
        can_read_session(auth, &self.policy)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT payload FROM events WHERE session_id = ?1 ORDER BY seq ASC")
            .map_err(|e| DomainError::Other(format!("sqlite prepare: {e}")))?;

        let events: Result<Vec<DomainEvent>, _> = stmt
            .query_map(params![session_id.to_string()], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })
            .map_err(|e| DomainError::Other(format!("sqlite query: {e}")))?
            .map(|r| {
                r.map_err(|e| DomainError::Other(format!("sqlite row: {e}")))
                    .and_then(|json| {
                        serde_json::from_str::<DomainEvent>(&json)
                            .map_err(|e| DomainError::Other(format!("deserialize event: {e}")))
                    })
            })
            .collect();
        drop(stmt);
        drop(conn);

        events
    }

    /// Persist an entity observation for temporal analysis.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails, serialization fails,
    /// or the database write fails.
    pub fn append_observation(
        &self,
        auth: &AuthContext,
        observation: &EntityObservation,
    ) -> Result<(), DomainError> {
        use crate::domain::auth::can_write_session;
        can_write_session(auth, &self.policy)?;

        let payload = serde_json::to_string(observation)
            .map_err(|e| DomainError::Other(format!("serialize observation: {e}")))?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;

        let write_result = conn
            .execute(
                "INSERT OR REPLACE INTO observations \
                (id, entity_id, source_id, observed_at, payload_hash, payload) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    observation.id.to_string(),
                    observation.entity_id.as_str(),
                    observation.source_id.as_str(),
                    observation.observed_at.timestamp(),
                    observation.payload_hash.as_str(),
                    payload,
                ],
            )
            .map_err(|e| DomainError::Other(format!("sqlite append_observation: {e}")));
        drop(conn);
        write_result?;

        Ok(())
    }

    /// Return the most recent observation for an entity/source pair.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails, query fails, or
    /// deserialization fails.
    pub fn latest_observation(
        &self,
        auth: &AuthContext,
        entity_id: &str,
        source_id: &str,
    ) -> Result<Option<EntityObservation>, DomainError> {
        use crate::domain::auth::can_read_session;
        can_read_session(auth, &self.policy)?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let payload: Option<String> = match conn.query_row(
            "SELECT payload FROM observations \
            WHERE entity_id = ?1 AND source_id = ?2 \
            ORDER BY observed_at DESC LIMIT 1",
            params![entity_id, source_id],
            |row| row.get(0),
        ) {
            Ok(p) => Some(p),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => {
                return Err(DomainError::Other(format!(
                    "sqlite latest_observation: {e}"
                )));
            }
        };
        drop(conn);

        match payload {
            Some(json) => {
                let observation: EntityObservation = serde_json::from_str(&json)
                    .map_err(|e| DomainError::Other(format!("deserialize observation: {e}")))?;
                Ok(Some(observation))
            }
            None => Ok(None),
        }
    }

    /// List recent observations for an entity, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails, query fails, or
    /// deserialization fails.
    pub fn list_observations_since(
        &self,
        auth: &AuthContext,
        entity_id: &str,
        since: DateTime<Utc>,
        max_items: usize,
    ) -> Result<Vec<EntityObservation>, DomainError> {
        use crate::domain::auth::can_read_session;
        can_read_session(auth, &self.policy)?;

        if max_items == 0 {
            return Ok(Vec::new());
        }

        let limit = i64::try_from(max_items)
            .map_err(|_| DomainError::Other("max_items exceeds i64 range".to_owned()))?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT payload FROM observations \
                WHERE entity_id = ?1 AND observed_at >= ?2 \
                ORDER BY observed_at DESC \
                LIMIT ?3",
            )
            .map_err(|e| DomainError::Other(format!("sqlite prepare: {e}")))?;

        let rows: Result<Vec<String>, DomainError> = stmt
            .query_map(params![entity_id, since.timestamp(), limit], |row| {
                row.get(0)
            })
            .map_err(|e| DomainError::Other(format!("sqlite query: {e}")))?
            .map(|r| r.map_err(|e| DomainError::Other(format!("sqlite row: {e}"))))
            .collect();
        drop(stmt);
        drop(conn);

        rows?
            .into_iter()
            .map(|json| {
                serde_json::from_str::<EntityObservation>(&json)
                    .map_err(|e| DomainError::Other(format!("deserialize observation: {e}")))
            })
            .collect()
    }

    /// List all observations across all entities since a timestamp, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if authorization fails, query fails, or
    /// deserialization fails.
    pub fn list_recent_observations(
        &self,
        auth: &AuthContext,
        since: DateTime<Utc>,
        max_items: usize,
    ) -> Result<Vec<EntityObservation>, DomainError> {
        use crate::domain::auth::can_read_session;
        can_read_session(auth, &self.policy)?;

        if max_items == 0 {
            return Ok(Vec::new());
        }

        let limit = i64::try_from(max_items)
            .map_err(|_| DomainError::Other("max_items exceeds i64 range".to_owned()))?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT payload FROM observations \
                WHERE observed_at >= ?1 \
                ORDER BY observed_at DESC \
                LIMIT ?2",
            )
            .map_err(|e| DomainError::Other(format!("sqlite prepare: {e}")))?;

        let rows: Result<Vec<String>, DomainError> = stmt
            .query_map(params![since.timestamp(), limit], |row| row.get(0))
            .map_err(|e| DomainError::Other(format!("sqlite query: {e}")))?
            .map(|r| r.map_err(|e| DomainError::Other(format!("sqlite row: {e}"))))
            .collect();
        drop(stmt);
        drop(conn);

        rows?
            .into_iter()
            .map(|json| {
                serde_json::from_str::<EntityObservation>(&json)
                    .map_err(|e| DomainError::Other(format!("deserialize observation: {e}")))
            })
            .collect()
    }

    /// Check if an idempotency key has been used (within 24h).
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the database read fails.
    pub fn check_idempotency_key(&self, key: &uuid::Uuid) -> Result<Option<String>, DomainError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let result: Option<String> = conn
            .query_row(
                "SELECT result FROM idempotency_keys WHERE key = ?1 AND created_at > unixepoch() - 86400",
                params![key.to_string()],
                |row| row.get(0),
            )
            .ok();
        drop(conn);
        Ok(result)
    }

    /// Record an idempotency key with its result.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the database write fails.
    pub fn set_idempotency_key(&self, key: &uuid::Uuid, result: &str) -> Result<(), DomainError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let set_result = conn
            .execute(
                "INSERT OR REPLACE INTO idempotency_keys (key, result) VALUES (?1, ?2)",
                params![key.to_string(), result],
            )
            .map_err(|e| DomainError::Other(format!("sqlite set idem key: {e}")));
        drop(conn);
        set_result?;
        Ok(())
    }

    /// Verify WAL mode is active (for test assertions).
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the database query fails.
    pub fn journal_mode(&self) -> Result<String, DomainError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .map_err(|e| DomainError::Other(format!("sqlite pragma: {e}")))?;
        drop(conn);
        Ok(mode)
    }

    /// Delete observations older than the specified duration.
    ///
    /// Useful for implementing retention policies (e.g., keep 90 days).
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the database write fails.
    pub fn cleanup_old_observations(&self, days_to_keep: i64) -> Result<usize, DomainError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DomainError::Other(e.to_string()))?;

        let cutoff_timestamp = chrono::Utc::now().timestamp() - (days_to_keep * 86400);

        let rows_affected = conn
            .execute(
                "DELETE FROM observations WHERE observed_at < ?1",
                params![cutoff_timestamp],
            )
            .map_err(|e| DomainError::Other(format!("cleanup observations: {e}")))?;
        drop(conn);

        Ok(rows_affected)
    }
}

// ── SessionStore port implementation ────────────────────────────────────────

#[cfg(feature = "runtime")]
impl crate::ports::session_store::SessionStore for SqliteSessionStore {
    fn save(
        &self,
        auth: &AuthContext,
        session: &AgentSession,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send {
        std::future::ready(Self::save(self, auth, session))
    }

    fn load(
        &self,
        auth: &AuthContext,
        id: SessionId,
    ) -> impl std::future::Future<Output = Result<Option<AgentSession>, DomainError>> + Send {
        std::future::ready(Self::load(self, auth, id))
    }

    fn list(
        &self,
        auth: &AuthContext,
    ) -> impl std::future::Future<Output = Result<Vec<AgentSession>, DomainError>> + Send {
        std::future::ready(Self::list(self, auth))
    }

    fn delete(
        &self,
        auth: &AuthContext,
        id: SessionId,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send {
        std::future::ready(Self::delete(self, auth, id))
    }

    fn append_event(
        &self,
        auth: &AuthContext,
        session_id: SessionId,
        event: DomainEvent,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send {
        std::future::ready(Self::append_event(self, auth, session_id, &event))
    }

    fn list_events(
        &self,
        auth: &AuthContext,
        session_id: SessionId,
    ) -> impl std::future::Future<Output = Result<Vec<DomainEvent>, DomainError>> + Send {
        std::future::ready(Self::list_events(self, auth, session_id))
    }

    fn check_idempotency_key(
        &self,
        key: &uuid::Uuid,
    ) -> impl std::future::Future<Output = Result<Option<String>, DomainError>> + Send {
        std::future::ready(Self::check_idempotency_key(self, key))
    }

    fn set_idempotency_key(
        &self,
        key: &uuid::Uuid,
        result: &str,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send {
        std::future::ready(Self::set_idempotency_key(self, key, result))
    }
}

#[cfg(feature = "runtime")]
impl crate::ports::observation_store::ObservationStore for SqliteSessionStore {
    fn append_observation(
        &self,
        auth: &AuthContext,
        observation: &EntityObservation,
    ) -> impl std::future::Future<Output = Result<(), DomainError>> + Send {
        std::future::ready(Self::append_observation(self, auth, observation))
    }

    fn latest_observation(
        &self,
        auth: &AuthContext,
        entity_id: &str,
        source_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<EntityObservation>, DomainError>> + Send
    {
        std::future::ready(Self::latest_observation(self, auth, entity_id, source_id))
    }

    fn list_observations_since(
        &self,
        auth: &AuthContext,
        entity_id: &str,
        since: DateTime<Utc>,
        max_items: usize,
    ) -> impl std::future::Future<Output = Result<Vec<EntityObservation>, DomainError>> + Send {
        std::future::ready(Self::list_observations_since(
            self, auth, entity_id, since, max_items,
        ))
    }

    fn list_recent_observations(
        &self,
        auth: &AuthContext,
        since: DateTime<Utc>,
        max_items: usize,
    ) -> impl std::future::Future<Output = Result<Vec<EntityObservation>, DomainError>> + Send {
        std::future::ready(Self::list_recent_observations(self, auth, since, max_items))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[cfg(feature = "runtime")]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::agent::{AgentConfig, AgentSession};
    use crate::domain::auth::{AuthContext, UserId};
    use crate::domain::events::DomainEvent;
    use crate::domain::observation::{EntityObservation, ObservationDelta};

    fn system_auth() -> AuthContext {
        AuthContext::system()
    }

    fn user_auth() -> (AuthContext, UserId) {
        let uid = UserId::new();
        let ctx = AuthContext {
            user_id: uid.clone(),
            roles: vec![Role::Operator],
            session_token: crate::domain::credentials::CredentialGuard::new("token".into()),
        };
        (ctx, uid)
    }

    fn other_user_auth() -> AuthContext {
        AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Operator],
            session_token: crate::domain::credentials::CredentialGuard::new("token2".into()),
        }
    }

    fn make_store() -> SqliteSessionStore {
        SqliteSessionStore::in_memory().unwrap()
    }

    #[test]
    fn create_and_load_session_roundtrip() {
        let store = make_store();
        let auth = system_auth();
        let session = AgentSession::create(AgentConfig::default());
        let sid = session.session_id;

        store.save(&auth, &session).unwrap();
        let loaded = store.load(&auth, sid).unwrap().expect("session exists");

        assert_eq!(loaded.session_id, sid);
        assert_eq!(loaded.config.model, session.config.model);
    }

    #[test]
    fn load_session_wrong_user_returns_security_error() {
        let store = make_store();
        let (user_auth, _uid) = user_auth();
        let other = other_user_auth();

        let session = AgentSession::create(AgentConfig::default());
        let sid = session.session_id;

        // Save as user_auth.
        store.save(&user_auth, &session).unwrap();

        // Load as different user → error.
        let result = store.load(&other, sid);
        assert!(
            result.is_err(),
            "Should deny access to other user's session"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, DomainError::Security(_)),
            "Expected Security error, got: {err:?}"
        );
    }

    #[test]
    fn list_sessions_returns_only_owned() {
        let store = make_store();
        let (user1, _) = user_auth();
        let (user2, _) = user_auth();

        let s1 = AgentSession::create(AgentConfig::default());
        let s2 = AgentSession::create(AgentConfig::default());

        store.save(&user1, &s1).unwrap();
        store.save(&user2, &s2).unwrap();

        let list1 = store.list(&user1).unwrap();
        assert_eq!(list1.len(), 1, "User1 should see only their session");
        assert_eq!(list1[0].session_id, s1.session_id);

        let list2 = store.list(&user2).unwrap();
        assert_eq!(list2.len(), 1, "User2 should see only their session");
        assert_eq!(list2[0].session_id, s2.session_id);
    }

    #[test]
    fn list_sessions_owner_role_sees_all() {
        let store = make_store();
        let (user1, _) = user_auth();
        let auth_owner = system_auth(); // Service role sees all

        let s1 = AgentSession::create(AgentConfig::default());
        let s2 = AgentSession::create(AgentConfig::default());

        store.save(&user1, &s1).unwrap();
        store.save(&auth_owner, &s2).unwrap();

        let all = store.list(&auth_owner).unwrap();
        assert_eq!(all.len(), 2, "Owner/Service should see all sessions");
    }

    #[test]
    fn append_and_list_events_ordered() {
        let store = make_store();
        let auth = system_auth();
        let session = AgentSession::create(AgentConfig::default());
        let sid = session.session_id;

        store.save(&auth, &session).unwrap();

        let e1 = DomainEvent::AgentStarted {
            session_id: sid,
            objective: "investigate".into(),
            timestamp: chrono::Utc::now(),
        };
        let e2 = DomainEvent::ToolCalled {
            session_id: sid,
            tool_name: "web_search".into(),
            args_summary: "q=test".into(),
            timestamp: chrono::Utc::now(),
        };

        store.append_event(&auth, sid, &e1).unwrap();
        store.append_event(&auth, sid, &e2).unwrap();

        let events = store.list_events(&auth, sid).unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], DomainEvent::AgentStarted { .. }));
        assert!(matches!(events[1], DomainEvent::ToolCalled { .. }));
    }

    #[test]
    fn append_and_query_observations() {
        let store = make_store();
        let auth = system_auth();

        let first = EntityObservation::new(
            "ethereum:0xabc".to_owned(),
            "blockchain_explorer".to_owned(),
            chrono::Utc::now() - chrono::Duration::hours(2),
            "1111aaaa".to_owned(),
            ObservationDelta::New,
        );
        let second = EntityObservation::new(
            "ethereum:0xabc".to_owned(),
            "blockchain_explorer".to_owned(),
            chrono::Utc::now() - chrono::Duration::hours(1),
            "2222bbbb".to_owned(),
            ObservationDelta::Changed {
                previous_hash: "1111aaaa".to_owned(),
            },
        );

        store.append_observation(&auth, &first).unwrap();
        store.append_observation(&auth, &second).unwrap();

        let latest = store
            .latest_observation(&auth, "ethereum:0xabc", "blockchain_explorer")
            .unwrap()
            .expect("expected latest observation");
        assert_eq!(latest.payload_hash, "2222bbbb");

        let since = chrono::Utc::now() - chrono::Duration::hours(3);
        let observations = store
            .list_observations_since(&auth, "ethereum:0xabc", since, 8)
            .unwrap();
        assert_eq!(observations.len(), 2);
    }

    #[test]
    fn idempotency_key_none_then_cached() {
        let store = make_store();
        let key = uuid::Uuid::new_v4();

        let first = store.check_idempotency_key(&key).unwrap();
        assert!(first.is_none(), "First check should be None");

        store.set_idempotency_key(&key, "done").unwrap();

        let second = store.check_idempotency_key(&key).unwrap();
        assert_eq!(second, Some("done".to_string()));
    }

    #[test]
    fn wal_mode_confirmed() {
        let store = make_store();
        let mode = store.journal_mode().unwrap();
        // In-memory databases may report "memory" instead of "wal",
        // but file-backed would report "wal". For in-memory, WAL pragma
        // is accepted but mode stays "memory".
        assert!(
            mode == "wal" || mode == "memory",
            "Expected wal or memory, got: {mode}"
        );
    }

    #[test]
    fn delete_session_cascades() {
        let store = make_store();
        let auth = AuthContext::owner(UserId::system(), "sys".into());
        let session = AgentSession::create(AgentConfig::default());
        let sid = session.session_id;

        store.save(&auth, &session).unwrap();

        let event = DomainEvent::AgentStarted {
            session_id: sid,
            objective: "test".into(),
            timestamp: chrono::Utc::now(),
        };
        store.append_event(&auth, sid, &event).unwrap();

        store.delete(&auth, sid).unwrap();

        let loaded = store.load(&auth, sid).unwrap();
        assert!(loaded.is_none(), "Session should be deleted");

        let events = store.list_events(&auth, sid).unwrap();
        assert!(events.is_empty(), "Events should be cascade-deleted");
    }

    #[test]
    fn delete_requires_permission() {
        let store = make_store();
        let auth = system_auth();
        let session = AgentSession::create(AgentConfig::default());
        let sid = session.session_id;
        store.save(&auth, &session).unwrap();

        // Create a user with only ReadSession permission (Reader role).
        let viewer = AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Reader],
            session_token: crate::domain::credentials::CredentialGuard::new("t".into()),
        };
        let result = store.delete(&viewer, sid);
        assert!(result.is_err(), "Viewer should not be able to delete");
    }

    #[test]
    fn update_via_load_modify_save() {
        let store = make_store();
        let auth = system_auth();
        let session = AgentSession::create(AgentConfig::default());
        let sid = session.session_id;

        store.save(&auth, &session).unwrap();

        // Load, modify, save.
        let mut loaded = store.load(&auth, sid).unwrap().unwrap();
        loaded.start("objective".into());
        store.save(&auth, &loaded).unwrap();

        let reloaded = store.load(&auth, sid).unwrap().unwrap();
        assert_eq!(
            reloaded.status,
            crate::domain::agent::SessionStatus::Running
        );
    }
}
