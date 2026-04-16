use clap::Parser;
use redshank_fetchers::fetchers::epa_superfund::fetch_superfund_site;

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
    match fetch_superfund_site(&args.query, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-epa-superfund failed: {err}");
            std::process::exit(1);
        }
    }
}
