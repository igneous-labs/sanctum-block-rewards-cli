use crate::{input_string, input_with_validation, validate_pubkey, ENDORSE_MESSAGE};
use clap::{command, Args};
use colored::Colorize;
use solana_sdk::signature::Signature;

#[derive(Args, Debug)]
#[command(long_about = "Verify validator signed message")]
pub struct VerifyArgs {}

impl VerifyArgs {
    pub async fn run(_args: crate::Args) {
        let identity_pubkey = input_with_validation(
            "Enter the Identity public key",
            "ETVqa6damHxVTEgy88YRHuaKfwggE7soxAKcqos5maur",
            None,
            None,
            validate_pubkey,
        );
        if identity_pubkey.is_err() {
            println!("{}", "Error: Invalid pubkey".red());
            return;
        }

        let identity_pubkey = identity_pubkey.unwrap();

        let signed_message = input_string("Enter signed message", "5KZiXZsDZ1...", None, None);
        if signed_message.is_err() {
            println!("{}", "Error: Invalid signed message".red());
            return;
        }
        let signed_message = signed_message.unwrap();

        println!("{}", "=".repeat(80));

        let signature = bs58::decode(signed_message.to_string())
            .into_vec()
            .ok()
            .and_then(|bytes| Signature::try_from(&bytes[..]).ok());

        if signature.is_none() {
            println!("{}", "Error: Invalid signature".red());
            return;
        }

        let signature = signature.unwrap();

        let verified = signature.verify(&identity_pubkey.to_bytes(), ENDORSE_MESSAGE.as_bytes());

        if verified {
            println!("{}", "✓ Verified!".green().bold());
        } else {
            println!("{}", "✗ Verification failed!".red().bold());
        }
    }
}
