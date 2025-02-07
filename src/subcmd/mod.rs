use clap::Subcommand;

mod transfer_rewards;

pub use transfer_rewards::*;

#[derive(Debug, Subcommand)]
pub enum Subcmd {
    TransferRewards(TransferRewardsArgs),
}

impl Subcmd {
    pub async fn run(args: crate::Args) {
        match args.subcmd {
            Self::TransferRewards(_) => TransferRewardsArgs::run(args).await,
        }
    }
}
