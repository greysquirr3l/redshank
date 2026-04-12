use clap::Parser;
use redshank_fetchers::fetchers::irs_990_xml::fetch_irs_990_xml;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    ein: String,
    #[arg(long)]
    year: u32,
    #[arg(short, long, default_value = ".")]
    output: std::path::PathBuf,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match fetch_irs_990_xml(&args.ein, args.year, &args.output).await {
        Ok(result) => println!("{}", result.output_path.display()),
        Err(err) => {
            eprintln!("fetch-irs-990-xml failed: {err}");
            std::process::exit(1);
        }
    }
}
