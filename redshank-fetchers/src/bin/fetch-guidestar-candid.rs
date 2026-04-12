use clap::Parser;
use redshank_fetchers::fetchers::guidestar_candid::fetch_guidestar_profile;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    ein: String,
    #[arg(long, env = "REDSHANK_CANDID_API_KEY")]
    api_key: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_guidestar_profile(&args.ein, &args.api_key, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-guidestar-candid failed: {err}");
            std::process::exit(1);
        }
    }
}
