use clap::Parser;
use redshank_fetchers::fetchers::blockchain_explorer::fetch_address_snapshot;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    chain: String,
    #[arg(long)]
    address: String,
    #[arg(long, env = "REDSHANK_ETHERSCAN_API_KEY")]
    api_key: Option<String>,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_address_snapshot(
        &args.chain,
        &args.address,
        args.api_key.as_deref(),
        &args.output,
    )
    .await
    {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-blockchain-explorer failed: {err}");
            std::process::exit(1);
        }
    }
}
