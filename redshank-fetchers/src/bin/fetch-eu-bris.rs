use clap::Parser;
use redshank_fetchers::fetchers::eu_bris::fetch_eu_bris;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    query: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_eu_bris(&args.query, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-eu-bris failed: {err}");
            std::process::exit(1);
        }
    }
}
