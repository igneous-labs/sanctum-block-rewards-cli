use clap::Subcommand;

mod calculate;
mod transfer;

pub use calculate::*;
pub use transfer::*;

#[derive(Debug, Subcommand)]
pub enum Subcmd {
    Transfer(TransferArgs),
    Calculate(CalculateArgs),
}

impl Subcmd {
    pub async fn run(args: crate::Args) {
        match args.subcmd {
            Self::Transfer(_) => TransferArgs::run(args).await,
            Self::Calculate(_) => CalculateArgs::run(args).await,
        }
    }
}
