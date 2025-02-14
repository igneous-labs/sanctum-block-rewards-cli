use crate::{input_string, input_with_validation, validate_pubkey, ENDORSE_MESSAGE};
use clap::{command, Args};
use colored::Colorize;
use solana_sdk::signature::Signature;

#[derive(Args, Debug)]
#[command(long_about = "Verify validator signed message")]
pub struct VerifyArgs {}

impl VerifyArgs {
    pub async fn run(_args: crate::Args) {
        let identity_pubkey = match input_with_validation(
            "Enter the Identity public key",
            "ETVqa6damHxVTEgy88YRHuaKfwggE7soxAKcqos5maur",
            None,
            None,
            validate_pubkey,
        ) {
            Ok(pubkey) => pubkey,
            Err(_) => {
                println!("{}", "Error: Invalid pubkey".red());
                return;
            }
        };

        let signed_message = match input_string("Enter signed message", "5KZiXZsDZ1...", None, None)
        {
            Ok(msg) => msg,
            Err(_) => {
                println!("{}", "Error: Invalid signed message".red());
                return;
            }
        };

        println!("{}", "=".repeat(80));

        let signature = match bs58::decode(signed_message.to_string())
            .into_vec()
            .ok()
            .and_then(|bytes| Signature::try_from(&bytes[..]).ok())
        {
            None => {
                println!("{}", "Error: Invalid signature".red());
                return;
            }
            Some(sig) => sig,
        };

        let verified = signature.verify(&identity_pubkey.to_bytes(), ENDORSE_MESSAGE.as_bytes());

        if verified {
            println!("{}", "✓ Verified!".green().bold());
        } else {
            println!("{}", "✗ Verification failed!".red().bold());
        }
    }
}
