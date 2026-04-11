//! File-based credential storage, `.env` parser, and multi-source resolution.
//!
//! Mirrors `agent/credentials.py` from the `OpenPlanter` Python implementation.

use crate::domain::credentials::{CredentialBundle, CredentialGuard};
use crate::domain::errors::DomainError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── .env parser ─────────────────────────────────────────────

/// Strip matching single or double quotes from a value.
fn strip_quotes(value: &str) -> &str {
    let v = value.trim();
    if v.len() >= 2
        && let bytes = v.as_bytes()
        && let (Some(&b0), Some(&blast)) = (bytes.first(), bytes.last())
        && (b0 == b'\'' || b0 == b'"')
        && b0 == blast
        && let Some(inner) = v.get(1..v.len() - 1)
    {
        return inner;
    }
    v
}

/// Parse a `.env` file into a key-value map.
///
/// Handles `KEY=value`, `KEY='value'`, `KEY="value"`, `export KEY=value`,
/// `#` comments, and blank lines.
#[must_use]
pub fn parse_env_file(path: &Path) -> HashMap<String, String> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };

    let mut env = HashMap::new();
    for raw in contents.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim();
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = strip_quotes(value.trim());
            if !key.is_empty() {
                env.insert(key.to_string(), value.to_string());
            }
        }
    }
    env
}

/// Build a `CredentialBundle` from a parsed env map.
///
/// Resolution order (highest priority first):
/// 1. `REDSHANK_<KEY>` — app-namespaced (use when running multiple agents)
/// 2. `OPENPLANTER_<KEY>` — legacy `OpenPlanter` backward compatibility
/// 3. `<KEY>` — bare/global env var
fn bundle_from_env_map(env: &HashMap<String, String>) -> CredentialBundle {
    let get = |redshank_key: &str,
               openplanter_key: &str,
               bare_key: &str|
     -> Option<CredentialGuard<String>> {
        let val = env
            .get(redshank_key)
            .or_else(|| env.get(openplanter_key))
            .or_else(|| env.get(bare_key))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        val.map(CredentialGuard::new)
    };

    let get_plain = |redshank_key: &str, openplanter_key: &str, bare_key: &str| -> Option<String> {
        env.get(redshank_key)
            .or_else(|| env.get(openplanter_key))
            .or_else(|| env.get(bare_key))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    CredentialBundle {
        openai_api_key: get(
            "REDSHANK_OPENAI_API_KEY",
            "OPENPLANTER_OPENAI_API_KEY",
            "OPENAI_API_KEY",
        ),
        anthropic_api_key: get(
            "REDSHANK_ANTHROPIC_API_KEY",
            "OPENPLANTER_ANTHROPIC_API_KEY",
            "ANTHROPIC_API_KEY",
        ),
        openrouter_api_key: get(
            "REDSHANK_OPENROUTER_API_KEY",
            "OPENPLANTER_OPENROUTER_API_KEY",
            "OPENROUTER_API_KEY",
        ),
        cerebras_api_key: get(
            "REDSHANK_CEREBRAS_API_KEY",
            "OPENPLANTER_CEREBRAS_API_KEY",
            "CEREBRAS_API_KEY",
        ),
        exa_api_key: get(
            "REDSHANK_EXA_API_KEY",
            "OPENPLANTER_EXA_API_KEY",
            "EXA_API_KEY",
        ),
        voyage_api_key: get(
            "REDSHANK_VOYAGE_API_KEY",
            "OPENPLANTER_VOYAGE_API_KEY",
            "VOYAGE_API_KEY",
        ),
        hibp_api_key: get(
            "REDSHANK_HIBP_API_KEY",
            "OPENPLANTER_HIBP_API_KEY",
            "HIBP_API_KEY",
        ),
        github_token: get(
            "REDSHANK_GITHUB_TOKEN",
            "OPENPLANTER_GITHUB_TOKEN",
            "GITHUB_TOKEN",
        ),
        fec_api_key: get(
            "REDSHANK_FEC_API_KEY",
            "OPENPLANTER_FEC_API_KEY",
            "FEC_API_KEY",
        ),
        opencorporates_api_key: get(
            "REDSHANK_OPENCORPORATES_API_KEY",
            "OPENPLANTER_OPENCORPORATES_API_KEY",
            "OPENCORPORATES_API_KEY",
        ),
        uk_companies_house_api_key: get(
            "REDSHANK_UK_COMPANIES_HOUSE_API_KEY",
            "OPENPLANTER_UK_COMPANIES_HOUSE_API_KEY",
            "UK_COMPANIES_HOUSE_API_KEY",
        ),
        opensanctions_api_key: get(
            "REDSHANK_OPENSANCTIONS_API_KEY",
            "OPENPLANTER_OPENSANCTIONS_API_KEY",
            "OPENSANCTIONS_API_KEY",
        ),
        ollama_base_url: get_plain(
            "REDSHANK_OLLAMA_BASE_URL",
            "OPENPLANTER_OLLAMA_BASE_URL",
            "OLLAMA_BASE_URL",
        ),
    }
}

/// Build a `CredentialBundle` from a `.env` file.
#[must_use]
pub fn credentials_from_env_file(path: &Path) -> CredentialBundle {
    let env = parse_env_file(path);
    bundle_from_env_map(&env)
}

// ── Environment variable source ─────────────────────────────

/// Build a `CredentialBundle` from process environment variables.
///
/// Resolution order (highest priority first):
/// 1. `REDSHANK_<KEY>` — app-namespaced (use when running multiple agents)
/// 2. `OPENPLANTER_<KEY>` — legacy `OpenPlanter` backward compatibility
/// 3. `<KEY>` — bare/global env var
#[must_use]
pub fn credentials_from_env() -> CredentialBundle {
    let get = |redshank_key: &str,
               openplanter_key: &str,
               bare_key: &str|
     -> Option<CredentialGuard<String>> {
        let val = std::env::var(redshank_key)
            .ok()
            .or_else(|| std::env::var(openplanter_key).ok())
            .or_else(|| std::env::var(bare_key).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        val.map(CredentialGuard::new)
    };

    let get_plain = |redshank_key: &str, openplanter_key: &str, bare_key: &str| -> Option<String> {
        std::env::var(redshank_key)
            .ok()
            .or_else(|| std::env::var(openplanter_key).ok())
            .or_else(|| std::env::var(bare_key).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    CredentialBundle {
        openai_api_key: get(
            "REDSHANK_OPENAI_API_KEY",
            "OPENPLANTER_OPENAI_API_KEY",
            "OPENAI_API_KEY",
        ),
        anthropic_api_key: get(
            "REDSHANK_ANTHROPIC_API_KEY",
            "OPENPLANTER_ANTHROPIC_API_KEY",
            "ANTHROPIC_API_KEY",
        ),
        openrouter_api_key: get(
            "REDSHANK_OPENROUTER_API_KEY",
            "OPENPLANTER_OPENROUTER_API_KEY",
            "OPENROUTER_API_KEY",
        ),
        cerebras_api_key: get(
            "REDSHANK_CEREBRAS_API_KEY",
            "OPENPLANTER_CEREBRAS_API_KEY",
            "CEREBRAS_API_KEY",
        ),
        exa_api_key: get(
            "REDSHANK_EXA_API_KEY",
            "OPENPLANTER_EXA_API_KEY",
            "EXA_API_KEY",
        ),
        voyage_api_key: get(
            "REDSHANK_VOYAGE_API_KEY",
            "OPENPLANTER_VOYAGE_API_KEY",
            "VOYAGE_API_KEY",
        ),
        hibp_api_key: get(
            "REDSHANK_HIBP_API_KEY",
            "OPENPLANTER_HIBP_API_KEY",
            "HIBP_API_KEY",
        ),
        github_token: get(
            "REDSHANK_GITHUB_TOKEN",
            "OPENPLANTER_GITHUB_TOKEN",
            "GITHUB_TOKEN",
        ),
        fec_api_key: get(
            "REDSHANK_FEC_API_KEY",
            "OPENPLANTER_FEC_API_KEY",
            "FEC_API_KEY",
        ),
        opencorporates_api_key: get(
            "REDSHANK_OPENCORPORATES_API_KEY",
            "OPENPLANTER_OPENCORPORATES_API_KEY",
            "OPENCORPORATES_API_KEY",
        ),
        uk_companies_house_api_key: get(
            "REDSHANK_UK_COMPANIES_HOUSE_API_KEY",
            "OPENPLANTER_UK_COMPANIES_HOUSE_API_KEY",
            "UK_COMPANIES_HOUSE_API_KEY",
        ),
        opensanctions_api_key: get(
            "REDSHANK_OPENSANCTIONS_API_KEY",
            "OPENPLANTER_OPENSANCTIONS_API_KEY",
            "OPENSANCTIONS_API_KEY",
        ),
        ollama_base_url: get_plain(
            "REDSHANK_OLLAMA_BASE_URL",
            "OPENPLANTER_OLLAMA_BASE_URL",
            "OLLAMA_BASE_URL",
        ),
    }
}

// ── JSON file credential store ──────────────────────────────

/// File-based credential store that reads/writes JSON with `chmod 600`.
#[derive(Debug, Clone)]
pub struct FileCredentialStore {
    /// Path to the `credentials.json` file.
    credentials_path: PathBuf,
}

impl FileCredentialStore {
    /// Workspace-level credential store at `<workspace>/.redshank/credentials.json`.
    #[must_use]
    pub fn workspace(workspace: &Path) -> Self {
        let credentials_path = workspace.join(".redshank").join("credentials.json");
        Self { credentials_path }
    }

    /// User-level credential store at `~/.redshank/credentials.json`.
    #[must_use]
    pub fn user_level() -> Self {
        let home = dirs_path();
        let credentials_path = home.join(".redshank").join("credentials.json");
        Self { credentials_path }
    }

    /// Path to the credentials file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.credentials_path
    }

    /// Load credentials from the JSON file. Returns an empty bundle on error.
    #[must_use]
    pub fn load(&self) -> CredentialBundle {
        let Ok(contents) = std::fs::read_to_string(&self.credentials_path) else {
            return CredentialBundle::default();
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    /// Save credentials to JSON and set file permissions to `0o600`.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the directory cannot be created, the file cannot be
    /// written, or permissions cannot be set.
    pub fn save(&self, bundle: &CredentialBundle) -> Result<(), DomainError> {
        if let Some(parent) = self.credentials_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DomainError::Other(format!(
                    "failed to create credential directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        let json = serde_json::to_string_pretty(bundle)
            .map_err(|e| DomainError::Other(format!("failed to serialize credentials: {e}")))?;

        std::fs::write(&self.credentials_path, &json).map_err(|e| {
            DomainError::Other(format!(
                "failed to write credentials to {}: {e}",
                self.credentials_path.display()
            ))
        })?;

        set_owner_only_perms(&self.credentials_path)?;

        Ok(())
    }
}

/// Set file permissions to owner read/write only (0o600) on Unix.
#[cfg(unix)]
fn set_owner_only_perms(path: &Path) -> Result<(), DomainError> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms).map_err(|e| {
        DomainError::Other(format!(
            "failed to set permissions on {}: {e}",
            path.display()
        ))
    })
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
fn set_owner_only_perms(_path: &Path) -> Result<(), DomainError> {
    Ok(())
}

/// Get the user's home directory.
fn dirs_path() -> PathBuf {
    std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from)
}

// ── Multi-source resolution ─────────────────────────────────

/// Resolve credentials from all sources in priority order.
///
/// Resolution order (highest priority wins):
/// 1. Explicit CLI args (passed in as `explicit`)
/// 2. `OPENPLANTER_*` or bare provider environment variables
/// 3. `.env` file in workspace root
/// 4. `.redshank/credentials.json` in workspace
/// 5. `~/.redshank/credentials.json` (user-level)
#[must_use]
pub fn resolve_credentials(
    workspace: &Path,
    explicit: Option<&CredentialBundle>,
) -> CredentialBundle {
    // Start with highest priority
    let mut bundle = explicit.cloned().unwrap_or_default();

    // 2. Environment variables
    let env_bundle = credentials_from_env();
    bundle.merge_missing(&env_bundle);

    // 3. .env file
    let env_file = workspace.join(".env");
    if env_file.is_file() {
        let dotenv_bundle = credentials_from_env_file(&env_file);
        bundle.merge_missing(&dotenv_bundle);
    }

    // 4. Workspace-level credential store
    let ws_store = FileCredentialStore::workspace(workspace);
    let ws_bundle = ws_store.load();
    bundle.merge_missing(&ws_bundle);

    // 5. User-level credential store
    let user_store = FileCredentialStore::user_level();
    let user_bundle = user_store.load();
    bundle.merge_missing(&user_bundle);

    bundle
}

/// Discover candidate `.env` file paths for a workspace.
#[must_use]
pub fn discover_env_candidates(workspace: &Path) -> Vec<PathBuf> {
    vec![workspace.join(".env")]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_env_handles_all_formats() {
        let dir = tempfile::tempdir().unwrap();
        let env_path = dir.path().join(".env");
        let mut f = std::fs::File::create(&env_path).unwrap();
        writeln!(f, "# This is a comment").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "BARE_KEY=bare_value").unwrap();
        writeln!(f, "SINGLE_QUOTED='single value'").unwrap();
        writeln!(f, "DOUBLE_QUOTED=\"double value\"").unwrap();
        writeln!(f, "export EXPORTED=exported_value").unwrap();
        writeln!(f, "  SPACED  =  spaced_value  ").unwrap();
        drop(f);

        let env = parse_env_file(&env_path);
        assert_eq!(env.get("BARE_KEY").unwrap(), "bare_value");
        assert_eq!(env.get("SINGLE_QUOTED").unwrap(), "single value");
        assert_eq!(env.get("DOUBLE_QUOTED").unwrap(), "double value");
        assert_eq!(env.get("EXPORTED").unwrap(), "exported_value");
        assert_eq!(env.get("SPACED").unwrap(), "spaced_value");
    }

    #[test]
    fn parse_env_skips_comments_and_blanks() {
        let dir = tempfile::tempdir().unwrap();
        let env_path = dir.path().join(".env");
        std::fs::write(&env_path, "# comment\n\nKEY=value\n").unwrap();

        let env = parse_env_file(&env_path);
        assert_eq!(env.len(), 1);
        assert_eq!(env.get("KEY").unwrap(), "value");
    }

    #[test]
    fn parse_env_missing_file_returns_empty() {
        let env = parse_env_file(Path::new("/nonexistent/.env"));
        assert!(env.is_empty());
    }

    #[test]
    fn credentials_from_env_file_maps_keys() {
        let dir = tempfile::tempdir().unwrap();
        let env_path = dir.path().join(".env");
        std::fs::write(
            &env_path,
            "ANTHROPIC_API_KEY=sk-ant-test\nOPENPLANTER_OPENAI_API_KEY=sk-op-test\nOLLAMA_BASE_URL=http://local:11434\n",
        )
        .unwrap();

        let bundle = credentials_from_env_file(&env_path);
        assert_eq!(
            bundle.anthropic_api_key.as_ref().unwrap().expose(),
            "sk-ant-test"
        );
        assert_eq!(
            bundle.openai_api_key.as_ref().unwrap().expose(),
            "sk-op-test"
        );
        assert_eq!(
            bundle.ollama_base_url.as_deref(),
            Some("http://local:11434")
        );
    }

    #[test]
    fn resolution_order_explicit_wins() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();

        // Write a .env file with a key
        let env_path = workspace.join(".env");
        std::fs::write(&env_path, "ANTHROPIC_API_KEY=from-env-file\n").unwrap();

        // Explicit arg should win
        let explicit = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("from-explicit".to_string())),
            ..Default::default()
        };

        let resolved = resolve_credentials(workspace, Some(&explicit));
        assert_eq!(
            resolved.anthropic_api_key.as_ref().unwrap().expose(),
            "from-explicit"
        );
    }

    #[test]
    fn file_credential_store_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileCredentialStore::workspace(dir.path());

        let bundle = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("sk-ant-persist".to_string())),
            ollama_base_url: Some("http://localhost:11434".to_string()),
            ..Default::default()
        };

        store.save(&bundle).unwrap();

        let loaded = store.load();
        assert_eq!(
            loaded.anthropic_api_key.as_ref().unwrap().expose(),
            "sk-ant-persist"
        );
        assert_eq!(
            loaded.ollama_base_url.as_deref(),
            Some("http://localhost:11434")
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_credential_store_sets_chmod_600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let store = FileCredentialStore::workspace(dir.path());

        let bundle = CredentialBundle::default();
        store.save(&bundle).unwrap();

        let meta = std::fs::metadata(store.path()).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "credentials file should be chmod 600, got {mode:o}"
        );
    }

    #[test]
    fn file_credential_store_load_missing_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileCredentialStore::workspace(dir.path());
        let loaded = store.load();
        assert!(!loaded.has_any());
    }

    #[test]
    fn env_file_bundle_fills_via_merge() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();

        // .env has openai key
        let env_path = workspace.join(".env");
        std::fs::write(&env_path, "OPENAI_API_KEY=from-dotenv\n").unwrap();

        // workspace store has anthropic key
        let ws_store = FileCredentialStore::workspace(workspace);
        let ws_bundle = CredentialBundle {
            anthropic_api_key: Some(CredentialGuard::new("from-ws-store".to_string())),
            ..Default::default()
        };
        ws_store.save(&ws_bundle).unwrap();

        let resolved = resolve_credentials(workspace, None);
        // Both should be populated (from different sources)
        assert!(resolved.openai_api_key.is_some() || std::env::var("OPENAI_API_KEY").is_ok());
        // The workspace-store value wins only when no higher-priority env var is set.
        // If the developer has a real ANTHROPIC_API_KEY set, that takes priority (level 2
        // vs level 4) — which is correct behaviour. We still assert the key is present.
        let no_env_anthropic = std::env::var("ANTHROPIC_API_KEY").is_err()
            && std::env::var("OPENPLANTER_ANTHROPIC_API_KEY").is_err();
        if no_env_anthropic {
            assert_eq!(
                resolved.anthropic_api_key.as_ref().unwrap().expose(),
                "from-ws-store"
            );
        } else {
            assert!(
                resolved.anthropic_api_key.is_some(),
                "anthropic key must be present from env or workspace store"
            );
        }
    }
}
