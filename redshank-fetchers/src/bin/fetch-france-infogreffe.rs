use clap::Parser;
use redshank_fetchers::fetchers::france_infogreffe::fetch_france_infogreffe;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    company_name: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_france_infogreffe(&args.company_name, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-france-infogreffe failed: {err}");
            std::process::exit(1);
        }
    }
}
