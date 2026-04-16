use clap::Parser;
use redshank_fetchers::fetchers::irs_1023::fetch_irs_1023_document;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    document_url: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_irs_1023_document(&args.document_url, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-irs-1023 failed: {err}");
            std::process::exit(1);
        }
    }
}
