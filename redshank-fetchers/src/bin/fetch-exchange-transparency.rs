use clap::Parser;
use redshank_fetchers::fetchers::exchange_transparency::fetch_exchange_transparency;

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
    match fetch_exchange_transparency(&args.url, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-exchange-transparency failed: {err}");
            std::process::exit(1);
        }
    }
}
