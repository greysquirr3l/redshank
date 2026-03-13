//! Tool dispatch adapters — `WorkspaceTools` implements `ToolDispatcher`.
//!
//! Sub-modules: filesystem, shell, web.
//! Patching (T11) and stygian (T12) are wired later.

#[cfg(feature = "runtime")]
mod filesystem;
#[cfg(feature = "runtime")]
mod shell;
#[cfg(feature = "runtime")]
mod web;

#[cfg(feature = "runtime")]
pub use workspace_tools::WorkspaceTools;

#[cfg(feature = "runtime")]
mod workspace_tools {
    use crate::domain::auth::{AuthContext, Permission, SecurityPolicy, StaticPolicy};
    use crate::domain::credentials::CredentialBundle;
    use crate::domain::errors::DomainError;
    use crate::domain::session::ToolResult;
    use crate::ports::tool_dispatcher::ToolDispatcher;
    use serde_json::Value;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    use super::filesystem;
    use super::shell::{self, BgJob};
    use super::web;

    /// Errors specific to tool execution (not domain errors).
    #[derive(Debug, thiserror::Error)]
    pub enum ToolError {
        #[error("path escapes workspace: {0}")]
        PathEscape(String),
        #[error("file not yet read: {0} — read it first")]
        PreReadGuard(String),
        #[error("heredoc syntax blocked by runtime policy")]
        Heredoc,
        #[error("interactive program blocked by runtime policy: {0}")]
        Interactive(String),
        #[error("parallel write conflict: {path} already claimed by {owner}")]
        ParallelWriteConflict { path: String, owner: String },
        #[error("{0}")]
        Other(String),
    }

    /// Parallel write group tracking.
    #[derive(Debug, Default)]
    pub struct WriteGroup {
        /// group_id → (path → owner_id)
        pub(super) claims: HashMap<String, HashMap<PathBuf, String>>,
    }

    /// WorkspaceTools: filesystem, shell, web tool dispatch.
    pub struct WorkspaceTools {
        /// Workspace root directory (canonicalized).
        pub(crate) root: PathBuf,
        /// Credential bundle for API keys.
        pub(crate) creds: CredentialBundle,
        /// Security policy.
        pub(crate) policy: Arc<dyn SecurityPolicy>,
        /// Background jobs.
        pub(crate) bg_jobs: Arc<Mutex<HashMap<u32, BgJob>>>,
        /// Next background job ID.
        pub(crate) bg_next_id: Arc<Mutex<u32>>,
        /// Files that have been read in this session.
        pub(crate) files_read: Arc<Mutex<HashSet<PathBuf>>>,
        /// Parallel write group tracking.
        pub(crate) write_group: Arc<Mutex<WriteGroup>>,
        /// Maximum characters for shell output.
        pub(crate) max_shell_output_chars: usize,
        /// Maximum characters when reading a file.
        pub(crate) max_file_chars: usize,
        /// Maximum files to list.
        pub(crate) max_files_listed: usize,
        /// Maximum search hits.
        pub(crate) max_search_hits: usize,
        /// Shell command timeout in seconds.
        pub(crate) command_timeout_secs: u64,
        /// Current execution scope (group_id, owner_id) for parallel writes.
        pub(crate) scope_group_id: Arc<Mutex<Option<String>>>,
        pub(crate) scope_owner_id: Arc<Mutex<Option<String>>>,
    }

    impl std::fmt::Debug for WorkspaceTools {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("WorkspaceTools")
                .field("root", &self.root)
                .finish_non_exhaustive()
        }
    }

    impl WorkspaceTools {
        /// Create a new `WorkspaceTools` instance.
        ///
        /// `root` is canonicalized on construction.
        pub fn new(root: PathBuf, creds: CredentialBundle) -> Result<Self, ToolError> {
            let canonical = root
                .canonicalize()
                .map_err(|e| ToolError::Other(format!("cannot canonicalize workspace: {e}")))?;
            if !canonical.is_dir() {
                return Err(ToolError::Other(format!(
                    "workspace is not a directory: {}",
                    canonical.display()
                )));
            }
            Ok(Self {
                root: canonical,
                creds,
                policy: Arc::new(StaticPolicy),
                bg_jobs: Arc::new(Mutex::new(HashMap::new())),
                bg_next_id: Arc::new(Mutex::new(1)),
                files_read: Arc::new(Mutex::new(HashSet::new())),
                write_group: Arc::new(Mutex::new(WriteGroup::default())),
                max_shell_output_chars: 16_000,
                max_file_chars: 32_000,
                max_files_listed: 400,
                max_search_hits: 200,
                command_timeout_secs: 120,
                scope_group_id: Arc::new(Mutex::new(None)),
                scope_owner_id: Arc::new(Mutex::new(None)),
            })
        }

        /// Set a custom security policy (for testing).
        pub fn with_policy(mut self, policy: Arc<dyn SecurityPolicy>) -> Self {
            self.policy = policy;
            self
        }

        /// Set command timeout.
        pub fn with_command_timeout(mut self, secs: u64) -> Self {
            self.command_timeout_secs = secs;
            self
        }

        /// Set max file chars.
        pub fn with_max_file_chars(mut self, chars: usize) -> Self {
            self.max_file_chars = chars;
            self
        }

        /// Resolve and validate a path within the workspace.
        pub(crate) fn resolve_path(&self, raw: &str) -> Result<PathBuf, ToolError> {
            let candidate = std::path::Path::new(raw);
            let full = if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                self.root.join(candidate)
            };
            // Use canonicalize for existing paths, otherwise canonicalize parent
            let resolved = if full.exists() {
                full.canonicalize()
                    .map_err(|e| ToolError::Other(format!("canonicalize failed: {e}")))?
            } else {
                // For new files: canonicalize parent, append filename
                let parent = full
                    .parent()
                    .ok_or_else(|| ToolError::PathEscape(raw.to_string()))?;
                if !parent.exists() {
                    // Check that the would-be parent is under root
                    // Walk up to find an existing ancestor
                    let mut ancestor = parent.to_path_buf();
                    while !ancestor.exists() {
                        ancestor = ancestor
                            .parent()
                            .ok_or_else(|| ToolError::PathEscape(raw.to_string()))?
                            .to_path_buf();
                    }
                    let canon_ancestor = ancestor.canonicalize().map_err(|e| {
                        ToolError::Other(format!("canonicalize failed: {e}"))
                    })?;
                    if !canon_ancestor.starts_with(&self.root) {
                        return Err(ToolError::PathEscape(raw.to_string()));
                    }
                    // Reconstruct path under root
                    let remaining = full.strip_prefix(&ancestor).map_err(|_| {
                        ToolError::PathEscape(raw.to_string())
                    })?;
                    canon_ancestor.join(remaining)
                } else {
                    let canon_parent = parent.canonicalize().map_err(|e| {
                        ToolError::Other(format!("canonicalize failed: {e}"))
                    })?;
                    let filename = full
                        .file_name()
                        .ok_or_else(|| ToolError::PathEscape(raw.to_string()))?;
                    canon_parent.join(filename)
                }
            };

            if resolved == self.root || resolved.starts_with(&self.root) {
                Ok(resolved)
            } else {
                Err(ToolError::PathEscape(raw.to_string()))
            }
        }

        /// Check if a write is allowed (pre-read guard).
        pub(crate) async fn check_write_allowed(&self, resolved: &PathBuf) -> Result<(), ToolError> {
            if resolved.exists() && resolved.is_file() {
                let read_set = self.files_read.lock().await;
                if !read_set.contains(resolved) {
                    let rel = resolved
                        .strip_prefix(&self.root)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| resolved.display().to_string());
                    return Err(ToolError::PreReadGuard(rel));
                }
            }
            Ok(())
        }

        /// Register a write target for parallel write conflict detection.
        pub(crate) async fn register_write_target(
            &self,
            resolved: &PathBuf,
        ) -> Result<(), ToolError> {
            let group_id = self.scope_group_id.lock().await.clone();
            let owner_id = self.scope_owner_id.lock().await.clone();

            if let (Some(gid), Some(oid)) = (group_id, owner_id) {
                let mut wg = self.write_group.lock().await;
                let claims = wg.claims.entry(gid).or_default();
                if let Some(existing_owner) = claims.get(resolved) {
                    if existing_owner != &oid {
                        let rel = resolved
                            .strip_prefix(&self.root)
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|_| resolved.display().to_string());
                        return Err(ToolError::ParallelWriteConflict {
                            path: rel,
                            owner: existing_owner.clone(),
                        });
                    }
                } else {
                    claims.insert(resolved.clone(), oid);
                }
            }
            Ok(())
        }

        /// Mark a file as read.
        pub(crate) async fn mark_read(&self, resolved: &std::path::Path) {
            self.files_read.lock().await.insert(resolved.to_path_buf());
        }

        /// Truncate text to max_chars with a truncation message.
        pub(crate) fn clip(text: &str, max_chars: usize) -> String {
            if text.len() <= max_chars {
                return text.to_string();
            }
            let omitted = text.len() - max_chars;
            format!(
                "{}\n\n...[truncated {omitted} chars]...",
                &text[..max_chars]
            )
        }

        /// Check permission via security policy, returning a ToolResult error if denied.
        fn check_permission(
            &self,
            auth: &AuthContext,
            permission: Permission,
        ) -> Result<(), DomainError> {
            self.policy.check(auth, permission)?;
            Ok(())
        }

        /// Determine which permission a tool requires.
        fn permission_for_tool(tool_name: &str) -> Permission {
            match tool_name {
                // Write tools
                "write_file" | "edit_file" | "apply_patch" | "hashline_edit"
                | "begin_parallel_write_group" | "end_parallel_write_group" => {
                    Permission::WriteSession
                }
                // Shell tools
                "run_shell" | "run_shell_bg" | "check_shell_bg" | "kill_shell_bg"
                | "cleanup_bg_jobs" => Permission::RunAgent,
                // Web tools
                "web_search" | "fetch_url" => Permission::FetchData,
                // Read tools
                _ => Permission::ReadSession,
            }
        }

        /// Begin a parallel write group.
        async fn begin_parallel_write_group(&self, args: &Value) -> String {
            let group_id = args
                .get("group_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            let mut wg = self.write_group.lock().await;
            wg.claims.entry(group_id.clone()).or_default();
            format!("Parallel write group '{group_id}' started.")
        }

        /// End a parallel write group.
        async fn end_parallel_write_group(&self, args: &Value) -> String {
            let group_id = args
                .get("group_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            let mut wg = self.write_group.lock().await;
            wg.claims.remove(&group_id);
            format!("Parallel write group '{group_id}' ended.")
        }
    }

    impl ToolDispatcher for WorkspaceTools {
        async fn dispatch(
            &self,
            auth: &AuthContext,
            tool_name: &str,
            arguments: Value,
        ) -> Result<ToolResult, DomainError> {
            // Check permission
            let permission = Self::permission_for_tool(tool_name);
            if let Err(e) = self.check_permission(auth, permission) {
                return Ok(ToolResult {
                    call_id: String::new(),
                    content: format!("Access denied: {e}"),
                    is_error: true,
                });
            }

            let content = match tool_name {
                // Filesystem
                "list_files" => filesystem::list_files(self, &arguments).await,
                "search_files" => filesystem::search_files(self, &arguments).await,
                "repo_map" => filesystem::repo_map(self, &arguments).await,
                "read_file" => filesystem::read_file(self, &arguments).await,
                "read_image" => filesystem::read_image(self, &arguments).await,
                "write_file" => filesystem::write_file(self, &arguments).await,
                "edit_file" => filesystem::edit_file(self, &arguments).await,
                "hashline_edit" => filesystem::hashline_edit(self, &arguments).await,
                // Shell
                "run_shell" => shell::run_shell(self, &arguments).await,
                "run_shell_bg" => shell::run_shell_bg(self, &arguments).await,
                "check_shell_bg" => shell::check_shell_bg(self, &arguments).await,
                "kill_shell_bg" => shell::kill_shell_bg(self, &arguments).await,
                "cleanup_bg_jobs" => shell::cleanup_bg_jobs(self).await,
                // Web
                "web_search" => web::web_search(self, &arguments).await,
                "fetch_url" => web::fetch_url(self, &arguments).await,
                // Parallel write groups
                "begin_parallel_write_group" => {
                    self.begin_parallel_write_group(&arguments).await
                }
                "end_parallel_write_group" => {
                    self.end_parallel_write_group(&arguments).await
                }
                // Patching — delegates to apply_patch in filesystem for now
                // TODO(T11): Codex-style patch format parser
                "apply_patch" => filesystem::apply_patch(self, &arguments).await,
                _ => format!("Unknown tool: {tool_name}"),
            };

            let is_error = content.starts_with("BLOCKED:")
                || content.starts_with("Access denied:")
                || content.starts_with("Failed")
                || content.starts_with("File not found:")
                || content.starts_with("Path escapes workspace:")
                || content.starts_with("Unknown tool:");

            Ok(ToolResult {
                call_id: String::new(),
                content,
                is_error,
            })
        }
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use super::WorkspaceTools;
    use crate::domain::auth::{AuthContext, Role, UserId};
    use crate::domain::credentials::{CredentialBundle, CredentialGuard};
    use crate::ports::tool_dispatcher::ToolDispatcher;
    use serde_json::json;

    fn test_creds() -> CredentialBundle {
        CredentialBundle {
            exa_api_key: Some(CredentialGuard::new("test-exa-key".to_string())),
            ..CredentialBundle::default()
        }
    }

    fn operator_auth() -> AuthContext {
        AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Operator],
            session_token: CredentialGuard::new("test-token".to_string()),
        }
    }

    fn reader_auth() -> AuthContext {
        AuthContext {
            user_id: UserId::new(),
            roles: vec![Role::Reader],
            session_token: CredentialGuard::new("test-token".to_string()),
        }
    }

    fn ws(dir: &tempfile::TempDir) -> WorkspaceTools {
        WorkspaceTools::new(dir.path().to_path_buf(), test_creds()).unwrap()
    }

    // ── Path escape guard ───────────────

    #[test]
    fn path_escape_is_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let result = tools.resolve_path("../../etc/passwd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("escapes workspace"));
    }

    #[test]
    fn relative_path_within_workspace_ok() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hi").unwrap();
        let tools = ws(&dir);
        let result = tools.resolve_path("hello.txt");
        assert!(result.is_ok());
    }

    // ── Write file to unread path (pre-read guard) ───────

    #[tokio::test]
    async fn write_file_to_unread_path_is_blocked() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("existing.txt"), "content").unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();
        let result = tools
            .dispatch(
                &auth,
                "write_file",
                json!({"path": "existing.txt", "content": "new content"}),
            )
            .await
            .unwrap();
        assert!(result.is_error, "should be blocked: {}", result.content);
        assert!(
            result.content.contains("not yet read") || result.content.contains("read it first"),
            "got: {}",
            result.content
        );
    }

    #[tokio::test]
    async fn write_file_after_read_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("existing.txt"), "old content").unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        // Read first
        let read_result = tools
            .dispatch(&auth, "read_file", json!({"path": "existing.txt"}))
            .await
            .unwrap();
        assert!(!read_result.is_error, "read failed: {}", read_result.content);

        // Now write
        let write_result = tools
            .dispatch(
                &auth,
                "write_file",
                json!({"path": "existing.txt", "content": "new content"}),
            )
            .await
            .unwrap();
        assert!(!write_result.is_error, "write failed: {}", write_result.content);
        assert!(write_result.content.contains("Wrote"));
    }

    #[tokio::test]
    async fn write_new_file_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        let result = tools
            .dispatch(
                &auth,
                "write_file",
                json!({"path": "new_file.txt", "content": "hello world"}),
            )
            .await
            .unwrap();
        assert!(!result.is_error, "write failed: {}", result.content);
        assert!(result.content.contains("Wrote"));

        let content = std::fs::read_to_string(dir.path().join("new_file.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    // ── Heredoc blocking ────────────────

    #[tokio::test]
    async fn run_shell_heredoc_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        let result = tools
            .dispatch(
                &auth,
                "run_shell",
                json!({"command": "cat << EOF\nhello\nEOF"}),
            )
            .await
            .unwrap();
        assert!(result.is_error, "should be blocked: {}", result.content);
        assert!(result.content.contains("BLOCKED"));
    }

    // ── Interactive blocking ────────────

    #[tokio::test]
    async fn run_shell_interactive_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        for cmd in &["vim file.txt", "nano file.txt", "less file.txt", "top", "htop"] {
            let result = tools
                .dispatch(&auth, "run_shell", json!({"command": cmd}))
                .await
                .unwrap();
            assert!(result.is_error, "should block {cmd}: {}", result.content);
            assert!(
                result.content.contains("BLOCKED"),
                "should block {cmd}: {}",
                result.content
            );
        }
    }

    // ── Shell timeout ───────────────────

    #[tokio::test]
    async fn run_shell_timeout_kills_process() {
        let dir = tempfile::tempdir().unwrap();
        let tools = WorkspaceTools::new(dir.path().to_path_buf(), test_creds())
            .unwrap()
            .with_command_timeout(1);
        let auth = operator_auth();

        let result = tools
            .dispatch(&auth, "run_shell", json!({"command": "sleep 60"}))
            .await
            .unwrap();
        assert!(
            result.content.contains("timeout"),
            "expected timeout: {}",
            result.content
        );
    }

    // ── Background jobs ─────────────────

    #[tokio::test]
    async fn bg_job_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        // Start a bg job
        let start = tools
            .dispatch(
                &auth,
                "run_shell_bg",
                json!({"command": "sleep 30"}),
            )
            .await
            .unwrap();
        assert!(!start.is_error, "start failed: {}", start.content);
        assert!(start.content.contains("job_id="));

        // Extract job_id
        let job_id: u32 = start
            .content
            .split("job_id=")
            .nth(1)
            .unwrap()
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap()
            .parse()
            .unwrap();

        // Check it (should be running)
        let check = tools
            .dispatch(
                &auth,
                "check_shell_bg",
                json!({"job_id": job_id}),
            )
            .await
            .unwrap();
        assert!(
            check.content.contains("running") || check.content.contains("finished"),
            "check: {}",
            check.content
        );

        // Kill it
        let kill = tools
            .dispatch(
                &auth,
                "kill_shell_bg",
                json!({"job_id": job_id}),
            )
            .await
            .unwrap();
        assert!(
            kill.content.contains("killed") || kill.content.contains("No background job"),
            "kill: {}",
            kill.content
        );
    }

    // ── Access control ──────────────────

    #[tokio::test]
    async fn reader_cannot_write() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = reader_auth();

        let result = tools
            .dispatch(
                &auth,
                "write_file",
                json!({"path": "test.txt", "content": "evil"}),
            )
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Access denied"));
    }

    #[tokio::test]
    async fn reader_cannot_run_shell() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = reader_auth();

        let result = tools
            .dispatch(&auth, "run_shell", json!({"command": "ls"}))
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Access denied"));
    }

    #[tokio::test]
    async fn reader_can_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hi there").unwrap();
        let tools = ws(&dir);
        let auth = reader_auth();

        let result = tools
            .dispatch(&auth, "read_file", json!({"path": "hello.txt"}))
            .await
            .unwrap();
        assert!(!result.is_error, "read failed: {}", result.content);
        assert!(result.content.contains("hi there"));
    }

    // ── Parallel write conflict ─────────

    #[tokio::test]
    async fn parallel_write_conflict_detected() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);

        // Start a parallel write group
        {
            let mut wg = tools.write_group.lock().await;
            wg.claims.entry("group1".to_string()).or_default();
        }

        // Simulate: owner-A claims a file
        let test_file = tools.resolve_path("shared.txt").unwrap();
        {
            let mut wg = tools.write_group.lock().await;
            let claims = wg.claims.get_mut("group1").unwrap();
            claims.insert(test_file.clone(), "owner-A".to_string());
        }

        // Now set scope to owner-B and try to register the same file
        *tools.scope_group_id.lock().await = Some("group1".to_string());
        *tools.scope_owner_id.lock().await = Some("owner-B".to_string());

        let result = tools.register_write_target(&test_file).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("conflict") || err.to_string().contains("claimed"));
    }

    // ── list_files ──────────────────────

    #[tokio::test]
    async fn list_files_returns_results() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "aaa").unwrap();
        std::fs::write(dir.path().join("b.rs"), "bbb").unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        let result = tools
            .dispatch(&auth, "list_files", json!({}))
            .await
            .unwrap();
        assert!(!result.is_error, "list failed: {}", result.content);
        assert!(result.content.contains("a.txt"));
        assert!(result.content.contains("b.rs"));
    }

    // ── edit_file ───────────────────────

    #[tokio::test]
    async fn edit_file_replaces_text() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello world").unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        // Read first
        tools
            .dispatch(&auth, "read_file", json!({"path": "test.txt"}))
            .await
            .unwrap();

        // Edit
        let result = tools
            .dispatch(
                &auth,
                "edit_file",
                json!({
                    "path": "test.txt",
                    "old_text": "hello world",
                    "new_text": "goodbye world"
                }),
            )
            .await
            .unwrap();
        assert!(!result.is_error, "edit failed: {}", result.content);
        assert!(result.content.contains("Edited"));

        let content = std::fs::read_to_string(dir.path().join("test.txt")).unwrap();
        assert_eq!(content, "goodbye world");
    }

    // ── run_shell basic ─────────────────

    #[tokio::test]
    async fn run_shell_echo() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        let result = tools
            .dispatch(&auth, "run_shell", json!({"command": "echo hello"}))
            .await
            .unwrap();
        assert!(!result.is_error, "shell failed: {}", result.content);
        assert!(result.content.contains("hello"));
    }

    // ── web_search mock ─────────────────
    // web_search/fetch_url require network; integration tests later.
    // We test that missing exa key returns an error.

    #[tokio::test]
    async fn web_search_without_key() {
        let dir = tempfile::tempdir().unwrap();
        let creds = CredentialBundle::default(); // no exa key
        let tools = WorkspaceTools::new(dir.path().to_path_buf(), creds).unwrap();
        let auth = operator_auth();

        let result = tools
            .dispatch(&auth, "web_search", json!({"query": "test"}))
            .await
            .unwrap();
        // Should mention missing key
        assert!(
            result.content.contains("EXA_API_KEY") || result.content.contains("not configured"),
            "got: {}",
            result.content
        );
    }

    // ── cleanup_bg_jobs ─────────────────

    #[tokio::test]
    async fn cleanup_bg_jobs_clears_all() {
        let dir = tempfile::tempdir().unwrap();
        let tools = ws(&dir);
        let auth = operator_auth();

        // Start two bg jobs
        tools
            .dispatch(
                &auth,
                "run_shell_bg",
                json!({"command": "sleep 60"}),
            )
            .await
            .unwrap();
        tools
            .dispatch(
                &auth,
                "run_shell_bg",
                json!({"command": "sleep 60"}),
            )
            .await
            .unwrap();

        // Cleanup
        let result = tools
            .dispatch(&auth, "cleanup_bg_jobs", json!({}))
            .await
            .unwrap();
        assert!(!result.is_error, "cleanup failed: {}", result.content);
        assert!(result.content.contains("killed") || result.content.contains("cleaned"));

        // Verify empty
        let jobs = tools.bg_jobs.lock().await;
        assert!(jobs.is_empty());
    }
}
