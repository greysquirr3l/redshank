// Transitive dependency version conflicts are outside our control.
#![allow(clippy::multiple_crate_versions)]
//! Redshank CLI — thin entry point.
//!
//! Constructs commands from CLI args and dispatches them to core handlers.
//! Zero business logic in this layer.

mod setup;

use clap::{Parser, Subcommand};
use redshank_core::adapters::persistence::credential_store::{
    FileCredentialStore, resolve_credentials,
};
use redshank_core::adapters::persistence::replay_log::FileReplayLogger;
use redshank_core::adapters::persistence::settings_store::SettingsStore;
use redshank_core::adapters::persistence::sqlite::SqliteSessionStore;
use redshank_core::adapters::providers::builder::{
    build_provider_with_settings, infer_provider, list_models_with_settings,
};
use redshank_core::adapters::tools::WorkspaceTools;
use redshank_core::application::commands::run_investigation::{
    IdempotencyKey, RunInvestigationCommand, RunInvestigationHandler,
};
use redshank_core::application::services::session_runtime::SessionRuntime;
use redshank_core::domain::agent::{AgentConfig, ProviderKind, ReasoningEffort};
use redshank_core::domain::auth::{AuthContext, UserId};
use redshank_core::domain::credentials::CredentialGuard;
use redshank_core::domain::errors::DomainError;
use redshank_core::domain::session::{SessionId, ToolResult};
use redshank_core::ports::tool_dispatcher::ToolDispatcher;
use redshank_fetchers::fetchers::uk_corporate_intelligence::fetch_uk_corporate_intelligence;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

const GIT_SHA: &str = env!("REDSHANK_GIT_SHA");

#[derive(Debug)]
struct EventingWorkspaceTools {
    inner: WorkspaceTools,
    tx: mpsc::Sender<redshank_tui::domain::AppEvent>,
}

impl ToolDispatcher for EventingWorkspaceTools {
    async fn dispatch(
        &self,
        auth: &redshank_core::domain::auth::AuthContext,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolResult, DomainError> {
        let _ = self
            .tx
            .send(redshank_tui::domain::AppEvent::ToolStart(
                tool_name.to_string(),
            ))
            .await;

        let result = self.inner.dispatch(auth, tool_name, arguments).await;

        let summary = match &result {
            Ok(tool_result) => summarize_tool_result(&tool_result.content, tool_result.is_error),
            Err(err) => format!("error: {err}"),
        };

        let _ = self
            .tx
            .send(redshank_tui::domain::AppEvent::ToolEnd(
                tool_name.to_string(),
                summary,
            ))
            .await;

        result
    }
}

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
            cmd_run(
                &workspace,
                &cli.model,
                &cli.reasoning,
                &objective,
                cli.max_depth,
                cli.demo,
            )
            .await
        }
        Commands::Tui { session } => {
            cmd_tui(
                &workspace,
                &cli.model,
                &cli.reasoning,
                session.as_deref(),
                cli.no_tui,
                cli.max_depth,
                cli.demo,
            )
            .await
        }
        Commands::Fetch {
            source,
            output,
            query,
        } => cmd_fetch(&workspace, &source, output.as_deref(), query.as_deref()).await,
        Commands::Session { action } => cmd_session(action, &workspace).await,
        Commands::Configure => cmd_configure(&workspace),
        Commands::Version => {
            println!("redshank {} ({})", env!("CARGO_PKG_VERSION"), GIT_SHA,);
            Ok(())
        }
    }
}

async fn cmd_run(
    workspace: &Path,
    model: &str,
    reasoning: &str,
    objective: &str,
    max_depth: u32,
    demo: bool,
) -> anyhow::Result<()> {
    tracing::info!(
        objective,
        model,
        reasoning,
        max_depth,
        demo,
        "starting headless investigation"
    );
    let answer = solve_objective(
        workspace,
        model,
        parse_core_reasoning(reasoning)?,
        objective,
        max_depth,
        demo,
    )
    .await?;
    println!("{answer}");
    Ok(())
}

#[allow(clippy::branches_sharing_code)]
async fn cmd_tui(
    workspace: &Path,
    model: &str,
    reasoning: &str,
    session_id: Option<&str>,
    no_tui: bool,
    max_depth: u32,
    demo: bool,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(256);
    let tui_reasoning = parse_tui_reasoning(reasoning)?;

    if let Some(id) = session_id {
        tracing::info!(session_id = id, "resuming session");
    }

    if no_tui {
        tracing::info!("running in headless (--no-tui) mode");
        drop(tx);
        redshank_tui::event_loop::run_headless(&mut rx).await;
        Ok(())
    } else {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let runtime_tx = tx.clone();
        let active_model = model.to_string();
        let active_reasoning = parse_core_reasoning(reasoning)?;
        let workspace_buf = workspace.to_path_buf();

        tokio::spawn(run_tui_command_loop(
            workspace_buf,
            active_model,
            active_reasoning,
            max_depth,
            demo,
            runtime_tx,
            command_rx,
        ));

        redshank_tui::event_loop::run(&mut rx, model.to_string(), tui_reasoning, Some(command_tx))
            .await?;
        Ok(())
    }
}

async fn run_tui_command_loop(
    workspace: PathBuf,
    initial_model: String,
    initial_reasoning: ReasoningEffort,
    max_depth: u32,
    demo: bool,
    runtime_tx: mpsc::Sender<redshank_tui::domain::AppEvent>,
    mut command_rx: mpsc::UnboundedReceiver<redshank_tui::domain::UiCommand>,
) {
    // Probe stygian availability once at startup and push the result to the TUI footer.
    let probe_cfg = redshank_fetchers::StygianProbeConfig::default();
    let fetcher_health = match redshank_fetchers::detect_stygian_availability(&probe_cfg).await {
        Ok(redshank_fetchers::StygianAvailability::Available { .. }) => {
            redshank_tui::domain::FetcherHealth::Up
        }
        _ => redshank_tui::domain::FetcherHealth::Down,
    };
    let _ = runtime_tx
        .send(redshank_tui::domain::AppEvent::FetcherHealthChanged(
            fetcher_health,
        ))
        .await;

    let mut active_model = initial_model;
    let mut active_reasoning = initial_reasoning;
    while let Some(cmd) = command_rx.recv().await {
        match cmd {
            redshank_tui::domain::UiCommand::SubmitObjective(objective) => {
                match solve_objective_with_events(
                    &workspace,
                    &active_model,
                    active_reasoning,
                    &objective,
                    max_depth,
                    demo,
                    runtime_tx.clone(),
                )
                .await
                {
                    Ok(answer) => {
                        if runtime_tx
                            .send(redshank_tui::domain::AppEvent::ContentDelta(answer))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        if runtime_tx
                            .send(redshank_tui::domain::AppEvent::AgentComplete(String::new()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(err) => {
                        if runtime_tx
                            .send(redshank_tui::domain::AppEvent::AgentComplete(format!(
                                "Run failed: {err}"
                            )))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            redshank_tui::domain::UiCommand::ListModels => {
                match list_models_for_active_provider(&workspace, &active_model).await {
                    Ok(listing) => {
                        if runtime_tx
                            .send(redshank_tui::domain::AppEvent::AgentComplete(listing))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(err) => {
                        if runtime_tx
                            .send(redshank_tui::domain::AppEvent::AgentComplete(format!(
                                "Model listing failed: {err}"
                            )))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            redshank_tui::domain::UiCommand::SetModel { name, .. } => {
                active_model = name;
            }
            redshank_tui::domain::UiCommand::SetReasoning(level) => {
                active_reasoning = tui_to_core_reasoning(level);
            }
            redshank_tui::domain::UiCommand::OpenWorkbench
            | redshank_tui::domain::UiCommand::CloseWorkbench => {
                // Workbench is a TUI-only UI concept; no CLI action needed.
            }
        }
    }
}

fn parse_core_reasoning(reasoning: &str) -> anyhow::Result<ReasoningEffort> {
    match reasoning.to_ascii_lowercase().as_str() {
        "off" | "none" => Ok(ReasoningEffort::None),
        "low" => Ok(ReasoningEffort::Low),
        "medium" | "med" => Ok(ReasoningEffort::Medium),
        "high" => Ok(ReasoningEffort::High),
        _ => anyhow::bail!("unsupported reasoning level: {reasoning}"),
    }
}

fn parse_tui_reasoning(reasoning: &str) -> anyhow::Result<redshank_tui::domain::ReasoningEffort> {
    redshank_tui::domain::ReasoningEffort::parse(reasoning)
        .ok_or_else(|| anyhow::anyhow!("unsupported reasoning level: {reasoning}"))
}

const fn tui_to_core_reasoning(level: redshank_tui::domain::ReasoningEffort) -> ReasoningEffort {
    match level {
        redshank_tui::domain::ReasoningEffort::Off => ReasoningEffort::None,
        redshank_tui::domain::ReasoningEffort::Low => ReasoningEffort::Low,
        redshank_tui::domain::ReasoningEffort::Medium => ReasoningEffort::Medium,
        redshank_tui::domain::ReasoningEffort::High => ReasoningEffort::High,
    }
}

fn build_agent_config(
    workspace: &Path,
    model: &str,
    reasoning_effort: ReasoningEffort,
    max_depth: u32,
    demo: bool,
) -> anyhow::Result<AgentConfig> {
    let provider = infer_provider(model)?;
    Ok(AgentConfig {
        workspace: workspace.to_path_buf(),
        provider,
        model: model.to_string(),
        reasoning_effort,
        max_depth: u8::try_from(max_depth.min(u32::from(u8::MAX))).unwrap_or(u8::MAX),
        demo_mode: demo,
        ..Default::default()
    })
}

fn replay_log_path(workspace: &Path) -> anyhow::Result<PathBuf> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("system clock before unix epoch: {e}"))?
        .as_secs();
    Ok(workspace
        .join(".redshank")
        .join("replays")
        .join(format!("session-{ts}.jsonl")))
}

async fn solve_objective(
    workspace: &Path,
    model: &str,
    reasoning_effort: ReasoningEffort,
    objective: &str,
    max_depth: u32,
    demo: bool,
) -> anyhow::Result<String> {
    let config = build_agent_config(workspace, model, reasoning_effort, max_depth, demo)?;
    let creds = resolve_credentials(workspace, None);
    let settings = SettingsStore::new(workspace).load();
    let provider = build_provider_with_settings(&config, &settings, &creds)?;
    let tools = WorkspaceTools::new(workspace, creds.clone())?
        .with_command_timeout(config.command_timeout.as_secs())
        .with_max_file_chars(config.max_file_chars);
    let replay_log = FileReplayLogger::new(replay_log_path(workspace)?);
    let db_path = workspace
        .join(".redshank")
        .join("sessions.db")
        .to_string_lossy()
        .into_owned();
    let store = SqliteSessionStore::open(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let cmd = RunInvestigationCommand {
        idempotency_key: IdempotencyKey::new(),
        session_id: SessionId::new(),
        objective: objective.to_owned(),
        config,
        auth: AuthContext::system(),
    };
    RunInvestigationHandler::new(store)
        .handle(cmd, provider, tools, replay_log)
        .await
        .map_err(Into::into)
}

async fn solve_objective_with_events(
    workspace: &Path,
    model: &str,
    reasoning_effort: ReasoningEffort,
    objective: &str,
    max_depth: u32,
    demo: bool,
    tx: mpsc::Sender<redshank_tui::domain::AppEvent>,
) -> anyhow::Result<String> {
    let config = build_agent_config(workspace, model, reasoning_effort, max_depth, demo)?;
    let creds = resolve_credentials(workspace, None);
    let settings = SettingsStore::new(workspace).load();
    let provider = build_provider_with_settings(&config, &settings, &creds)?;
    let tools = EventingWorkspaceTools {
        inner: WorkspaceTools::new(workspace, creds.clone())?
            .with_command_timeout(config.command_timeout.as_secs())
            .with_max_file_chars(config.max_file_chars),
        tx,
    };
    let replay_log = FileReplayLogger::new(replay_log_path(workspace)?);
    let db_path = workspace
        .join(".redshank")
        .join("sessions.db")
        .to_string_lossy()
        .into_owned();
    let store = SqliteSessionStore::open(&db_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let cmd = RunInvestigationCommand {
        idempotency_key: IdempotencyKey::new(),
        session_id: SessionId::new(),
        objective: objective.to_owned(),
        config,
        auth: AuthContext::system(),
    };
    RunInvestigationHandler::new(store)
        .handle(cmd, provider, tools, replay_log)
        .await
        .map_err(Into::into)
}

async fn list_models_for_active_provider(workspace: &Path, model: &str) -> anyhow::Result<String> {
    let provider = infer_provider(model)?;
    let creds = resolve_credentials(workspace, None);
    let settings = SettingsStore::new(workspace).load();
    let models = list_models_with_settings(provider, &settings, &creds).await?;
    Ok(format_model_listing(provider, model, &models))
}

fn format_model_listing(provider: ProviderKind, active_model: &str, models: &[String]) -> String {
    if models.is_empty() {
        return format!(
            "Available models for {}: none returned",
            provider_label(provider)
        );
    }

    let active_display = format_model_name_for_display(provider, active_model);
    let mut display_models: Vec<String> = models
        .iter()
        .map(|model| format_model_name_for_display(provider, model))
        .collect();
    display_models.sort();

    let contains_active = display_models.iter().any(|model| model == &active_display);

    let mut lines = if provider == ProviderKind::OpenRouter {
        format_grouped_openrouter_lines(&display_models, &active_display)
    } else {
        display_models
            .iter()
            .map(|model| format_model_line(model, &active_display))
            .collect::<Vec<_>>()
    };

    if !contains_active {
        lines.insert(0, format!("* {active_display} (active, unavailable)"));
    }

    format!(
        "Available models for {}:\n{}",
        provider_label(provider),
        lines.join("\n")
    )
}

fn format_model_name_for_display(provider: ProviderKind, model: &str) -> String {
    match provider {
        ProviderKind::OpenAiCompatible if !model.starts_with("ollama/") => {
            format!("ollama/{model}")
        }
        ProviderKind::OpenRouter if !model.starts_with("openrouter/") => {
            format!("openrouter/{model}")
        }
        ProviderKind::Cerebras if !model.starts_with("cerebras/") => format!("cerebras/{model}"),
        _ => model.to_string(),
    }
}

fn format_model_line(model: &str, active_display: &str) -> String {
    if model == active_display {
        format!("* {model} (active)")
    } else {
        format!("  {model}")
    }
}

fn format_grouped_openrouter_lines(models: &[String], active_display: &str) -> Vec<String> {
    let mut groups: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    for model in models {
        groups
            .entry(openrouter_group_label(model))
            .or_default()
            .push(model.clone());
    }

    let mut lines = Vec::new();
    for (group, group_models) in groups {
        lines.push(format!("{group}:"));
        for model in group_models {
            lines.push(format!("  {}", format_model_line(&model, active_display)));
        }
    }
    lines
}

fn openrouter_group_label(model: &str) -> String {
    model
        .strip_prefix("openrouter/")
        .unwrap_or(model)
        .split('/')
        .next()
        .unwrap_or("other")
        .to_string()
}

const fn provider_label(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::Anthropic => "Anthropic",
        ProviderKind::OpenAI => "OpenAI",
        ProviderKind::OpenRouter => "OpenRouter",
        ProviderKind::Cerebras => "Cerebras",
        ProviderKind::OpenAiCompatible => "OpenAI-Compatible",
    }
}

fn summarize_tool_result(content: &str, is_error: bool) -> String {
    let first_line = content.lines().next().unwrap_or_default().trim();
    let mut summary: String = first_line.chars().take(80).collect();
    if summary.is_empty() {
        summary = if is_error {
            "tool error".into()
        } else {
            "ok".into()
        };
    }
    if is_error {
        format!("error: {summary}")
    } else {
        summary
    }
}

async fn cmd_fetch(
    workspace: &Path,
    source: &str,
    output: Option<&std::path::Path>,
    query: Option<&str>,
) -> anyhow::Result<()> {
    tracing::info!(source, ?output, query, "fetching data");
    let output_dir = output.unwrap_or_else(|| std::path::Path::new("."));
    let query = query.ok_or_else(|| anyhow::anyhow!("--query is required for fetch"))?;

    match source {
        "uk_corporate_intelligence" | "uk-corporate-intelligence" => {
            let credentials = resolve_credentials(workspace, None);
            let companies_house_api_key = required_secret(
                credentials.uk_companies_house_api_key.as_ref(),
                "UK_COMPANIES_HOUSE_API_KEY",
            )?;
            let opencorporates_api_key = credentials
                .opencorporates_api_key
                .as_ref()
                .map(|s| s.expose().clone());
            let result = fetch_uk_corporate_intelligence(
                query,
                &companies_house_api_key,
                opencorporates_api_key.as_deref(),
                output_dir,
                500,
                25,
                2,
            )
            .await?;
            println!("{}", result.output_path.display());
            Ok(())
        }
        _ => anyhow::bail!("fetch dispatch not yet wired — source: {source}"),
    }
}

fn required_secret(value: Option<&CredentialGuard<String>>, name: &str) -> anyhow::Result<String> {
    value
        .map(|secret| secret.expose().clone())
        .filter(|secret| !secret.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing required credential: {name}"))
}

async fn cmd_session(action: SessionAction, workspace: &std::path::Path) -> anyhow::Result<()> {
    let db_dir = workspace.join(".redshank");
    std::fs::create_dir_all(&db_dir)?;
    let db_path = db_dir.join("sessions.db");
    let store = SqliteSessionStore::open(&db_path.to_string_lossy())?;
    // CLI user is the workspace Owner — full access to local sessions.
    let auth = AuthContext::owner(UserId::new(), "cli".to_string());
    let runtime = SessionRuntime::new(store);

    match action {
        SessionAction::List => {
            let sessions = runtime.list_sessions(auth).await?;
            if sessions.is_empty() {
                println!("No sessions found.");
            } else {
                println!("{:<40}  {:<12}  Model", "ID", "Status");
                println!("{}", "-".repeat(72));
                for s in &sessions {
                    println!(
                        "{:<40}  {:<12}  {}",
                        s.session_id,
                        format!("{:?}", s.status),
                        s.config.model,
                    );
                }
            }
            Ok(())
        }
        SessionAction::Delete { id } => {
            tracing::info!(session_id = %id, "deleting session");
            let uuid = uuid::Uuid::parse_str(&id)
                .map_err(|e| anyhow::anyhow!("invalid session ID '{id}': {e}"))?;
            let session_id = SessionId(uuid);
            runtime.delete_session(auth, session_id).await?;
            println!("Session {id} deleted.");
            Ok(())
        }
        SessionAction::Resume { id } => {
            tracing::info!(session_id = %id, "resuming session");
            // Resume is implemented as a TUI launch with the session pre-selected.
            drop(runtime);
            cmd_tui(
                workspace,
                "claude-sonnet-4-20250514",
                "medium",
                Some(&id),
                false,
                3,
                false,
            )
            .await
        }
    }
}

fn cmd_configure(workspace: &Path) -> anyhow::Result<()> {
    use setup::{ALL_CREDENTIAL_FIELDS, apply_input, fields_for_group, groups};

    let creds_path = workspace.join(".redshank").join("credentials.json");
    println!("Redshank — interactive credential setup");
    println!("Credentials will be saved to: {}", creds_path.display());
    println!("Leave any field empty to keep its current value.\n");

    // Load existing credentials so we can show [set] indicators.
    let mut bundle = FileCredentialStore::workspace(workspace).load();

    let prompt_plain = |label: &str, env_var: &str| -> anyhow::Result<Option<String>> {
        eprint!("{label} ({env_var}): ");
        io::stderr().flush().ok();
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let trimmed = line.trim().to_string();
        Ok(if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        })
    };

    for group in groups() {
        println!("\n── {group} ──");
        let fields = fields_for_group(group);
        for field in &fields {
            let already_set = if field.field_name == "ollama_base_url" {
                bundle.ollama_base_url.is_some()
            } else {
                bundle.has_field(field.field_name)
            };
            let indicator = if already_set { " [set]" } else { "" };
            let req = if field.is_required { " *required*" } else { "" };
            let secret_hint = if field.is_secret { " (secret)" } else { "" };
            eprintln!(
                "  {}{}{}{}  <{}>",
                field.label, indicator, req, secret_hint, field.signup_url
            );
            if let Some(value) = prompt_plain(field.label, field.env_var)? {
                apply_input(&mut bundle, field.field_name, value);
            }
        }
    }

    println!();
    // Remind operators about fields listed in ALL_CREDENTIAL_FIELDS.
    let total = ALL_CREDENTIAL_FIELDS.len();
    let set_count = ALL_CREDENTIAL_FIELDS
        .iter()
        .filter(|f| {
            if f.field_name == "ollama_base_url" {
                bundle.ollama_base_url.is_some()
            } else {
                bundle.has_field(f.field_name)
            }
        })
        .count();
    println!("{set_count}/{total} credentials configured.");

    if !bundle.has_any() {
        println!("No credentials provided — nothing saved.");
        return Ok(());
    }

    FileCredentialStore::workspace(workspace).save(&bundle)?;
    println!("Credentials saved successfully (0600 permissions).");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
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
        assert!(
            matches!(cli.command, Commands::Run { objective } if objective == "Investigate ACME Corp")
        );
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
    fn cli_parses_fetch_for_uk_corporate_intelligence() {
        let cli = Cli::try_parse_from([
            "redshank",
            "fetch",
            "uk_corporate_intelligence",
            "--query",
            "Acme Holdings",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Commands::Fetch { source, query, .. }
                if source == "uk_corporate_intelligence"
                    && query.as_deref() == Some("Acme Holdings")
        ));
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
        let cli = Cli::try_parse_from(["redshank", "tui", "--session", "my-session-id"]).unwrap();
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
        let pkg_version = env!("CARGO_PKG_VERSION");
        let version_str = format!("redshank {pkg_version} ({GIT_SHA})");
        assert!(version_str.starts_with("redshank "));
        assert!(version_str.contains(pkg_version));
        assert!(version_str.contains('('));
    }

    #[tokio::test]
    async fn session_list_empty_succeeds() {
        let workspace = std::path::Path::new("/tmp");
        let result = cmd_session(SessionAction::List, workspace).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn cmd_tui_no_tui_returns_without_hanging() {
        let workspace = std::path::Path::new("/tmp");
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            cmd_tui(workspace, "gpt-5.4", "medium", None, true, 5, false),
        )
        .await;

        assert!(result.is_ok(), "cmd_tui --no-tui should not hang");
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn parse_core_reasoning_maps_off_to_none() {
        assert_eq!(parse_core_reasoning("off").unwrap(), ReasoningEffort::None);
        assert_eq!(parse_core_reasoning("high").unwrap(), ReasoningEffort::High);
    }

    #[test]
    fn build_agent_config_infers_ollama_provider() {
        let config = build_agent_config(
            std::path::Path::new("/tmp"),
            "ollama/gemma3:27b",
            ReasoningEffort::Medium,
            5,
            false,
        )
        .unwrap();

        assert_eq!(config.model, "ollama/gemma3:27b");
        assert_eq!(
            config.provider,
            redshank_core::domain::agent::ProviderKind::OpenAiCompatible
        );
    }

    #[test]
    fn format_model_listing_prefixes_ollama_models() {
        let listing = format_model_listing(
            ProviderKind::OpenAiCompatible,
            "ollama/llama3:latest",
            &["llama3:latest".into(), "gemma3:27b".into()],
        );

        assert!(listing.contains("ollama/llama3:latest"));
        assert!(listing.contains("ollama/gemma3:27b"));
    }

    #[test]
    fn format_model_listing_prefixes_openrouter_models() {
        let listing = format_model_listing(
            ProviderKind::OpenRouter,
            "openrouter/anthropic/claude-3.7-sonnet",
            &["anthropic/claude-3.7-sonnet".into(), "openai/gpt-4o".into()],
        );

        assert!(listing.contains("openrouter/anthropic/claude-3.7-sonnet"));
        assert!(listing.contains("openrouter/openai/gpt-4o"));
    }

    #[test]
    fn format_model_listing_marks_active_model() {
        let listing = format_model_listing(
            ProviderKind::OpenAiCompatible,
            "ollama/gemma3:27b",
            &["llama3:latest".into(), "gemma3:27b".into()],
        );

        assert!(listing.contains("* ollama/gemma3:27b (active)"));
    }

    #[test]
    fn format_model_listing_groups_openrouter_models() {
        let listing = format_model_listing(
            ProviderKind::OpenRouter,
            "openrouter/openai/gpt-4o",
            &[
                "anthropic/claude-3.7-sonnet".into(),
                "openai/gpt-4o".into(),
                "openai/gpt-4o-mini".into(),
            ],
        );

        assert!(listing.contains("anthropic:"));
        assert!(listing.contains("openai:"));
        assert!(listing.contains("* openrouter/openai/gpt-4o (active)"));
    }
}
