use clap::Parser;
use tokio::runtime::Runtime;

fn main() {
    let args = sanctum_block_rewards_cli::Args::parse();
    let rt = Runtime::new().unwrap();
    rt.block_on(sanctum_block_rewards_cli::Subcmd::run(args));
}
