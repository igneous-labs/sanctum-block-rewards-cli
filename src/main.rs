use clap::Parser;
use tokio::runtime::Runtime;

fn main() {
    let args = sanctum_validator_apy_cli::Args::parse();
    let rt = Runtime::new().unwrap();
    rt.block_on(sanctum_validator_apy_cli::Subcmd::run(args));
}
