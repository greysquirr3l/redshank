use clap::Parser;
use redshank_fetchers::fetchers::sec_climate::fetch_sec_climate;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    cik: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_sec_climate(&args.cik, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-sec-climate failed: {err}");
            std::process::exit(1);
        }
    }
}
