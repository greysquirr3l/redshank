//! Redshank CLI — thin entry point.
//!
//! Constructs commands from CLI args and dispatches them to core handlers.
//! Zero business logic in this layer.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

const GIT_SHA: &str = env!("REDSHANK_GIT_SHA");

/// Redshank — autonomous recursive investigation agent.
#[derive(Parser, Debug)]
#[command(name = "redshank", version, about, long_about = None)]
struct Cli {
    /// Workspace directory (default: current directory).
    #[arg(long, global = true, env = "REDSHANK_WORKSPACE")]
    workspace: Option<PathBuf>,

    /// Model to use (e.g. "claude-sonnet-4-20250514").
    #[arg(long, global = true, default_value = "claude-sonnet-4-20250514")]
    model: String,

    /// Reasoning effort level.
    #[arg(long, global = true, default_value = "medium")]
    reasoning: String,

    /// Disable TUI (headless mode).
    #[arg(long, global = true)]
    no_tui: bool,

    /// Maximum recursion depth for sub-tasks.
    #[arg(long, global = true, default_value = "5")]
    max_depth: u32,

    /// Enable demo mode (redact real entity names in output).
    #[arg(long, global = true)]
    demo: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run an investigation headless (output to stdout).
    Run {
        /// Investigation objective / prompt.
        objective: String,
    },
    /// Launch the interactive TUI.
    Tui {
        /// Resume a specific session by ID.
        #[arg(long)]
        session: Option<String>,
    },
    /// Fetch data from a specific source.
    Fetch {
        /// Data source name (e.g. "sec-edgar", "fec", "ofac-sdn").
        source: String,
        /// Output directory for fetched data.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Query string.
        #[arg(long)]
        query: Option<String>,
    },
    /// Manage investigation sessions.
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Interactive credential setup.
    Configure,
    /// Print version info.
    Version,
}

#[derive(Subcommand, Debug)]
enum SessionAction {
    /// List all sessions.
    List,
    /// Delete a session by ID.
    Delete {
        /// Session ID to delete.
        id: String,
    },
    /// Resume a session by ID.
    Resume {
        /// Session ID to resume.
        id: String,
    },
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_logging();

    let workspace = cli
        .workspace
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    tracing::info!(
        "redshank v{version} ({sha}) — model: {model} — workspace: {path}",
        version = env!("CARGO_PKG_VERSION"),
        sha = GIT_SHA,
        model = cli.model,
        path = workspace.display(),
    );

    match cli.command {
        Commands::Run { objective } => {
            cmd_run(&workspace, &cli.model, &objective, cli.max_depth, cli.demo).await
        }
        Commands::Tui { session } => {
            cmd_tui(&workspace, &cli.model, session.as_deref(), cli.no_tui).await
        }
        Commands::Fetch {
            source,
            output,
            query,
        } => cmd_fetch(&source, output.as_deref(), query.as_deref()).await,
        Commands::Session { action } => cmd_session(action, &workspace).await,
        Commands::Configure => cmd_configure().await,
        Commands::Version => {
            println!(
                "redshank {} ({})",
                env!("CARGO_PKG_VERSION"),
                GIT_SHA,
            );
            Ok(())
        }
    }
}

async fn cmd_run(
    workspace: &std::path::Path,
    model: &str,
    objective: &str,
    max_depth: u32,
    demo: bool,
) -> anyhow::Result<()> {
    tracing::info!(
        objective,
        model,
        max_depth,
        demo,
        "starting headless investigation"
    );
    let _ = (workspace, objective, model, max_depth, demo);
    // TODO(T24): Wire to SessionRuntime.solve() once integration tests confirm the stack.
    anyhow::bail!("headless run not yet wired — see T24 integration tests")
}

async fn cmd_tui(
    _workspace: &std::path::Path,
    _model: &str,
    session_id: Option<&str>,
    no_tui: bool,
) -> anyhow::Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);

    if let Some(id) = session_id {
        tracing::info!(session_id = id, "resuming session");
    }

    if no_tui {
        tracing::info!("running in headless (--no-tui) mode");
        // Send a quit event so headless returns immediately in tests.
        let _ = tx;
        redshank_tui::event_loop::run_headless(&mut rx).await;
        Ok(())
    } else {
        // Spawn input reader in background
        let reader_tx = tx.clone();
        tokio::spawn(async move {
            redshank_tui::crossterm_reader::spawn_reader(reader_tx).await;
        });

        let _ = tx; // drop our sender so the loop can detect channel close

        redshank_tui::event_loop::run(&mut rx).await?;
        Ok(())
    }
}

async fn cmd_fetch(
    source: &str,
    output: Option<&std::path::Path>,
    query: Option<&str>,
) -> anyhow::Result<()> {
    tracing::info!(source, ?output, query, "fetching data");
    // TODO(T24): Dispatch to named fetcher
    anyhow::bail!("fetch dispatch not yet wired — source: {source}")
}

async fn cmd_session(action: SessionAction, workspace: &std::path::Path) -> anyhow::Result<()> {
    let _ = workspace;
    match action {
        SessionAction::List => {
            // TODO(T24): Query SqliteSessionStore
            println!("ID | Created | Last Objective");
            println!("---|---------|---------------");
            println!("(no sessions)");
            Ok(())
        }
        SessionAction::Delete { id } => {
            tracing::info!(session_id = %id, "deleting session");
            // TODO(T24): Wire to SqliteSessionStore.delete()
            anyhow::bail!("session delete not yet wired — id: {id}")
        }
        SessionAction::Resume { id } => {
            tracing::info!(session_id = %id, "resuming session");
            // TODO(T24): Wire to TUI with session resume
            anyhow::bail!("session resume not yet wired — id: {id}")
        }
    }
}

async fn cmd_configure() -> anyhow::Result<()> {
    // TODO(T24): Interactive rpassword prompts for credentials
    println!("Interactive credential setup — not yet implemented.");
    println!("Credentials are stored in ~/.redshank/credentials.json");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_parses_version_subcommand() {
        let cli = Cli::try_parse_from(["redshank", "version"]);
        assert!(cli.is_ok());
        assert!(matches!(cli.unwrap().command, Commands::Version));
    }

    #[test]
    fn cli_parses_run_with_objective() {
        let cli = Cli::try_parse_from(["redshank", "run", "Investigate ACME Corp"]).unwrap();
        assert!(matches!(cli.command, Commands::Run { objective } if objective == "Investigate ACME Corp"));
    }

    #[test]
    fn cli_parses_fetch_with_source_and_query() {
        let cli = Cli::try_parse_from([
            "redshank",
            "fetch",
            "sec-edgar",
            "--query",
            "ACME",
            "--output",
            "/tmp/out",
        ])
        .unwrap();
        match cli.command {
            Commands::Fetch {
                source,
                query,
                output,
            } => {
                assert_eq!(source, "sec-edgar");
                assert_eq!(query.as_deref(), Some("ACME"));
                assert_eq!(output, Some(PathBuf::from("/tmp/out")));
            }
            _ => panic!("expected Fetch"),
        }
    }

    #[test]
    fn cli_parses_session_list() {
        let cli = Cli::try_parse_from(["redshank", "session", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Session {
                action: SessionAction::List
            }
        ));
    }

    #[test]
    fn cli_parses_session_delete() {
        let cli = Cli::try_parse_from(["redshank", "session", "delete", "abc-123"]).unwrap();
        match cli.command {
            Commands::Session {
                action: SessionAction::Delete { id },
            } => assert_eq!(id, "abc-123"),
            _ => panic!("expected Session Delete"),
        }
    }

    #[test]
    fn cli_parses_session_resume() {
        let cli = Cli::try_parse_from(["redshank", "session", "resume", "abc-123"]).unwrap();
        match cli.command {
            Commands::Session {
                action: SessionAction::Resume { id },
            } => assert_eq!(id, "abc-123"),
            _ => panic!("expected Session Resume"),
        }
    }

    #[test]
    fn cli_parses_tui_with_session() {
        let cli =
            Cli::try_parse_from(["redshank", "tui", "--session", "my-session-id"]).unwrap();
        match cli.command {
            Commands::Tui { session } => assert_eq!(session.as_deref(), Some("my-session-id")),
            _ => panic!("expected Tui"),
        }
    }

    #[test]
    fn cli_parses_global_flags() {
        let cli = Cli::try_parse_from([
            "redshank",
            "--model",
            "gpt-4o",
            "--reasoning",
            "high",
            "--no-tui",
            "--max-depth",
            "10",
            "--demo",
            "version",
        ])
        .unwrap();
        assert_eq!(cli.model, "gpt-4o");
        assert_eq!(cli.reasoning, "high");
        assert!(cli.no_tui);
        assert_eq!(cli.max_depth, 10);
        assert!(cli.demo);
    }

    #[test]
    fn cli_parses_configure() {
        let cli = Cli::try_parse_from(["redshank", "configure"]).unwrap();
        assert!(matches!(cli.command, Commands::Configure));
    }

    #[test]
    fn git_sha_is_set() {
        assert!(!GIT_SHA.is_empty());
        assert_ne!(GIT_SHA, "unknown");
    }

    #[test]
    fn version_string_format() {
        let version_str = format!("redshank {} ({})", env!("CARGO_PKG_VERSION"), GIT_SHA);
        assert!(version_str.starts_with("redshank 0.1.0"));
        assert!(version_str.contains('('));
    }

    #[tokio::test]
    async fn session_list_empty_succeeds() {
        let workspace = std::path::Path::new("/tmp");
        let result = cmd_session(SessionAction::List, workspace).await;
        assert!(result.is_ok());
    }
}

