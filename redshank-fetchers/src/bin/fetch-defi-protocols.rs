use clap::Parser;
use redshank_fetchers::fetchers::defi_protocols::fetch_compound_positions;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    address: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_compound_positions(&args.address, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-defi-protocols failed: {err}");
            std::process::exit(1);
        }
    }
}
