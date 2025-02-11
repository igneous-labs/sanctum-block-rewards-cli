use clap::Subcommand;

mod calculate;
mod share;
mod transfer;

pub use calculate::*;
pub use share::*;
pub use transfer::*;

#[derive(Debug, Subcommand)]
pub enum Subcmd {
    Transfer(TransferArgs),
    Calculate(CalculateArgs),
    Share(ShareArgs),
}

impl Subcmd {
    pub async fn run(args: crate::Args) {
        match args.subcmd {
            Self::Transfer(_) => TransferArgs::run(args).await,
            Self::Calculate(_) => CalculateArgs::run(args).await,
            Self::Share(_) => ShareArgs::run(args).await,
        }
    }
}
