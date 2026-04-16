use clap::Parser;
use redshank_fetchers::fetchers::germany_handelsregister::fetch_germany_handelsregister;

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
    match fetch_germany_handelsregister(&args.query, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-germany-handelsregister failed: {err}");
            std::process::exit(1);
        }
    }
}
