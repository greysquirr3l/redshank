use clap::Parser;
use redshank_fetchers::fetchers::blockchain_explorer::BlockchainTransaction;
use redshank_fetchers::fetchers::tornado_screening::screen_transactions;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    address: String,
}

fn main() {
    let args = Args::parse();
    let result = screen_transactions(&args.address, &Vec::<BlockchainTransaction>::new());
    match serde_json::to_string_pretty(&result) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("fetch-tornado-screening failed: {err}");
            std::process::exit(1);
        }
    }
}
