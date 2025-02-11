use clap::Subcommand;

mod calculate;
mod sign;
mod transfer;
mod verify;

pub use calculate::*;
pub use sign::*;
pub use transfer::*;
pub use verify::*;

#[derive(Debug, Subcommand)]
pub enum Subcmd {
    Transfer(TransferArgs),
    Calculate(CalculateArgs),
    Sign(SignArgs),
    Verify(VerifyArgs),
}

impl Subcmd {
    pub async fn run(args: crate::Args) {
        match args.subcmd {
            Self::Transfer(_) => TransferArgs::run(args).await,
            Self::Calculate(_) => CalculateArgs::run(args).await,
            Self::Sign(_) => SignArgs::run(args).await,
            Self::Verify(_) => VerifyArgs::run(args).await,
        }
    }
}
