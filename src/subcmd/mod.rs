use clap::Subcommand;

mod calculate;
mod calculate_with_dune;
mod transfer;
mod calculate_apy;

pub use calculate::*;
pub use calculate_with_dune::*;
pub use transfer::*;
pub use calculate_apy::*;


#[derive(Debug, Subcommand)]
pub enum Subcmd {
    Transfer(TransferArgs),
    Calculate(CalculateArgs),
    CalculateWithDune(CalculateWithDuneArgs),
    CalculateAPY(CalculateAPYArgs)
}

impl Subcmd {
    pub async fn run(args: crate::Args) {
        match args.subcmd {
            Self::Transfer(_) => TransferArgs::run(args).await,
            Self::Calculate(_) => CalculateArgs::run(args).await,
            Self::CalculateWithDune(_) => CalculateWithDuneArgs::run(args).await,
            Self::CalculateAPY(_)=> CalculateAPYArgs::run(args).await
        }
    }
}
