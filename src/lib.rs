mod subcmd;

use clap::{builder::ValueParser, Parser};
use sanctum_solana_cli_utils::ConfigWrapper;
pub use subcmd::*;

#[derive(Parser, Debug)]
#[command(author, version, about = "Sanctum Block Rewards CLI")]
pub struct Args {
    #[arg(
        long,
        short,
        help = "Path to solana CLI config. Defaults to solana cli default if not provided",
        default_value = "",
        value_parser = ValueParser::new(ConfigWrapper::parse_from_path)
    )]
    pub config: ConfigWrapper,

    #[command(subcommand)]
    pub subcmd: Subcmd,
}
