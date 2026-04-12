use clap::Parser;
use redshank_fetchers::fetchers::uk_corporate_intelligence::fetch_uk_corporate_intelligence;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    query: String,
    #[arg(long, env = "REDSHANK_UK_COMPANIES_HOUSE_API_KEY")]
    companies_house_api_key: String,
    #[arg(long, env = "REDSHANK_OPENCORPORATES_API_KEY")]
    opencorporates_api_key: Option<String>,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
    #[arg(long, default_value_t = 500)]
    rate_limit_ms: u64,
    #[arg(long, default_value_t = 25)]
    max_results: u32,
    #[arg(long, default_value_t = 2)]
    max_pages: u32,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_uk_corporate_intelligence(
        &args.query,
        &args.companies_house_api_key,
        args.opencorporates_api_key.as_deref(),
        &args.output,
        args.rate_limit_ms,
        args.max_results,
        args.max_pages,
    )
    .await
    {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-uk-corporate-intelligence failed: {err}");
            std::process::exit(1);
        }
    }
}