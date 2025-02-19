use clap::Subcommand;

mod calculate;
mod calculate_with_dune;
mod transfer;

pub use calculate::*;
pub use calculate_with_dune::*;
pub use transfer::*;

#[derive(Debug, Subcommand)]
pub enum Subcmd {
    Transfer(TransferArgs),
    Calculate(CalculateArgs),
    CalculateWithDune(CalculateWithDuneArgs),
}

impl Subcmd {
    pub async fn run(args: crate::Args) {
        match args.subcmd {
            Self::Transfer(_) => TransferArgs::run(args).await,
            Self::Calculate(_) => CalculateArgs::run(args).await,
            Self::CalculateWithDune(_) => CalculateWithDuneArgs::run(args).await,
        }
    }
}
