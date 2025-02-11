use clap::{command, Args};
use colored::Colorize;
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner};

use crate::ENDORSE_MESSAGE;

use super::Subcmd;

#[derive(Args, Debug)]
#[command(long_about = "Deposit an activated stake account into a stake pool")]
pub struct SignArgs {
    #[arg(long, short, help = "The identity keypair for your validator")]
    pub identity_keypair_path: String,
}

impl SignArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_keypair_path,
        } = match args.subcmd {
            Subcmd::Sign(a) => a,
            _ => unreachable!(),
        };

        let identity_keypair = parse_named_signer(ParseNamedSigner {
            name: "identity",
            arg: &identity_keypair_path,
        })
        .unwrap();

        let signed_message = identity_keypair.sign_message(ENDORSE_MESSAGE.as_bytes());

        println!("{}", format!("Signed Message:").green().bold());
        println!("{}", signed_message);

        println!("{}", "=".repeat(80));

        println!(
            "{}",
            format!("Reach out to us on Telegram with your signed message @sanctumso")
                .blue()
                .bold()
        );
    }
}
