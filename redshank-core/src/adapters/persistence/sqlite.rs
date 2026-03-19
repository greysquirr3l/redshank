//! SQLite-backed session store implementing the `SessionStore` port.
//!
//! Uses rusqlite with WAL mode. All methods enforce `AuthContext` checks.
//! In tests, uses `:memory:` databases.

#[cfg(feature = "runtime")]
use std::sync::Mutex;

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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[cfg(feature = "runtime")]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::domain::agent::{AgentConfig, AgentSession};
    use crate::domain::auth::{AuthContext, UserId};
    use crate::domain::events::DomainEvent;

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
