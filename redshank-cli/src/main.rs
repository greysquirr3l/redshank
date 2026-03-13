//! Redshank CLI — thin entry point.
//!
//! Constructs CQRS Command/Query structs from CLI args and dispatches
//! them to redshank-core handlers. Zero business logic in this layer.

use clap::Parser;

/// Redshank — autonomous recursive investigation agent.
#[derive(Parser, Debug)]
#[command(name = "redshank", version, about)]
struct Cli {
    /// Investigation goal / prompt.
    #[arg(short, long)]
    goal: Option<String>,

    /// Model to use (e.g. "claude-sonnet-4-20250514").
    #[arg(short, long, default_value = "claude-sonnet-4-20250514")]
    model: String,
}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    // TODO(T23): Wire up TUI and agent engine
    println!("redshank — not yet implemented. See PROGRESS.md for task status.");
    Ok(())
}
