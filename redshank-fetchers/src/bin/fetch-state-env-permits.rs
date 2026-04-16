use clap::Parser;
use redshank_fetchers::fetchers::state_env_permits::fetch_state_permit;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    url: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_state_permit(&args.url, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-state-env-permits failed: {err}");
            std::process::exit(1);
        }
    }
}
