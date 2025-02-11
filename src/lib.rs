mod solana_utils;
mod subcmd;
mod utils;

use clap::Parser;
use sanctum_solana_cli_utils::TxSendMode;

use solana_sdk::commitment_config::CommitmentConfig;
pub use solana_utils::*;
pub use subcmd::*;
pub use utils::*;

pub const SOLANA_PUBLIC_RPC: &str = "https://api.mainnet-beta.solana.com";

#[derive(Parser, Debug)]
#[command(author, version, about = "Sanctum Block Rewards CLI")]
pub struct Args {
    #[arg(
        long,
        short,
        help = "RPC URL to use for all requests. Defaults to the Solana public RPC if not provided"
    )]
    pub rpc_url: Option<String>,

    #[arg(
        long,
        short,
        help = "Commitment level to use for RPC calls. Defaults to confirmed if not provided",
        default_value = "confirmed",
        value_enum
    )]
    pub commitment: Option<CommitmentConfig>,

    #[arg(
        long,
        short,
        help = "Transaction send mode.
- send-actual: signs and sends the tx to the cluster specified in config and outputs hash to stderr
- sim-only: simulates the tx against the cluster and outputs logs to stderr
- dump-msg: dumps the base64 encoded tx to stdout. For use with inspectors and multisigs
",
        default_value_t = TxSendMode::default(),
        value_enum,
    )]
    pub send_mode: TxSendMode,

    #[arg(
        long,
        short,
        help = "0 - disable ComputeBudgetInstruction prepending.
Any positive integer - enable dynamic compute budget calculation:
Before sending a TX, simulate the tx and prepend with appropriate ComputeBudgetInstructions.
This arg is the max priority fee the user will pay per transaction in lamports.
",
        default_value_t = 1
    )]
    pub fee_limit_cb: u64,

    #[command(subcommand)]
    pub subcmd: Subcmd,
}
