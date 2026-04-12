use clap::Parser;
use redshank_fetchers::fetchers::carbon_registries::fetch_carbon_project;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    project_id: String,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_carbon_project(&args.project_id, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-carbon-registries failed: {err}");
            std::process::exit(1);
        }
    }
}
