use crate::{
    get_leader_slots_for_identity, get_rewards_file_path, get_total_block_rewards_for_slots,
    input_string, input_with_validation, subcmd::Subcmd, validate_epoch, validate_rpc_url,
    SOLANA_PUBLIC_RPC,
};
use clap::{command, Args};
use colored::Colorize;
use inquire::Confirm;
use sanctum_solana_cli_utils::TokenAmt;
use serde_json::{json, Value};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use spinners::{Spinner, Spinners};
use std::{fs::File, path::Path, str::FromStr};

#[derive(Args, Debug)]
#[command(
    long_about = "Calculate the total block rewards earned by your validator for a specific epoch."
)]
pub struct CalculateArgs {
    #[arg(long, help = "The identity pubkey of your validator")]
    pub identity_pubkey: Option<String>,
    #[arg(long, help = "The epoch to calculate rewards for")]
    pub epoch: Option<u64>,
}

impl CalculateArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_pubkey,
            epoch,
        } = match args.subcmd {
            Subcmd::Calculate(args) => args,
            _ => unreachable!(),
        };

        let identity_pubkey = match input_string(
            "Enter your validator's identity key:",
            "Identity key",
            None,
            identity_pubkey,
        ) {
            Ok(key) => key,
            Err(_) => {
                println!("{}", "Error: Invalid identity key".red());
                return;
            }
        };

        let identity_pubkey = match Pubkey::from_str(&identity_pubkey) {
            Ok(pubkey) => pubkey,
            Err(_) => {
                println!("{}", "Error: Invalid identity pubkey".red());
                return;
            }
        };

        let rpc_url = match input_with_validation(
            "Enter the RPC URL:",
            "RPC URL",
            Some(SOLANA_PUBLIC_RPC.to_string()),
            args.rpc_url,
            validate_rpc_url,
        ) {
            Ok(url) => url,
            Err(_) => {
                println!("{}", "Error: Invalid RPC URL".red());
                return;
            }
        };

        let rpc = RpcClient::new_with_commitment(
            rpc_url,
            args.commitment.unwrap_or(CommitmentConfig::confirmed()),
        );

        let (current_epoch_info, epoch_schedule) =
            match tokio::try_join!(rpc.get_epoch_info(), rpc.get_epoch_schedule()) {
                Ok(result) => result,
                Err(_) => {
                    println!("{}", "Error: Failed to fetch data from RPC".red());
                    return;
                }
            };

        let epoch = match input_with_validation(
            "Enter the epoch to calculate rewards for:",
            &(current_epoch_info.epoch - 1).to_string(),
            Some((current_epoch_info.epoch - 1).to_string()),
            epoch.map(|e| e.to_string()),
            |input| validate_epoch(input, current_epoch_info.epoch),
        ) {
            Ok(e) => e,
            Err(_) => {
                println!("{}", "Error: Invalid epoch".red());
                return;
            }
        };
        println!("{}", "=".repeat(80));

        // Check if rewards file exists
        let rewards_file_path = match get_rewards_file_path(&identity_pubkey, epoch) {
            Ok(path) => path,
            Err(err) => {
                println!("{}", format!("Error: {}", err).red());
                return;
            }
        };

        // if path exists, read the file and display the total block rewards
        if Path::new(&rewards_file_path).exists() {
            let rewards: Value = match File::open(rewards_file_path.clone())
                .map_err(|_| "Failed to open rewards file")
                .and_then(|file| {
                    serde_json::from_reader(file).map_err(|_| "Failed to parse rewards file")
                }) {
                Ok(value) => value,
                Err(err) => {
                    println!("{}", format!("Error: {}", err).red());
                    return;
                }
            };

            let total_block_rewards = match rewards["total_block_rewards"].as_u64() {
                Some(rewards) => rewards,
                None => {
                    println!("{}", "Error: Invalid rewards file format".red());
                    return;
                }
            };

            println!(
                "{}",
                format!("Rewards file found at {}", rewards_file_path).blue()
            );
            println!(
                "{}",
                format!(
                    "✓ Total block rewards for {}... in epoch {} are {} SOL",
                    &identity_pubkey.to_string()[..6],
                    epoch,
                    TokenAmt {
                        amt: total_block_rewards,
                        decimals: 9
                    }
                )
                .green()
                .bold()
            );

            println!("{}", "=".repeat(80));
            return;
        }

        let mut sp = Spinner::new(
            Spinners::Dots,
            format!(
                "Fetching leader slots for {}...",
                &identity_pubkey.to_string()[..6]
            ),
        );

        let leader_slots =
            match get_leader_slots_for_identity(&rpc, epoch, &epoch_schedule, &identity_pubkey)
                .await
            {
                Ok(slots) => slots,
                Err(err) => {
                    println!("{}", format!("Error: {}", err).red());
                    return;
                }
            };

        let num_leader_slots = leader_slots.len();
        sp.stop_with_message(
            format!(
                "✓ Found {} leader slots for {}... in epoch {}",
                num_leader_slots,
                &identity_pubkey.to_string()[..6],
                epoch
            )
            .green()
            .bold()
            .to_string(),
        );

        if leader_slots.len() > 200 && rpc.url() == SOLANA_PUBLIC_RPC {
            println!(
                "{}",
                "⚠️ We recommend using a custom RPC URL to avoid longer wait time and rate limits."
                    .yellow()
                    .bold()
            );
            println!(
                "{}",
                "⚠️ We also have a `calculate-with-dune` command that also calulcates block rewards but using Dune Analytics."
                    .yellow()
                    .bold()
            );
        }

        println!("{}", "=".repeat(80));

        let ans = Confirm::new(
            &"Do you wish to continue with fetching block rewards?"
                .blue()
                .bold(),
        )
        .with_default(true)
        .prompt();

        match ans {
            Ok(false) => {
                return;
            }
            Err(_) => {
                println!("Error: Something went wrong.");
                return;
            }
            _ => (),
        }

        println!("{}", "=".repeat(80));

        let total_block_rewards = match get_total_block_rewards_for_slots(&rpc, &leader_slots).await
        {
            Ok(rewards) => rewards,
            Err(err) => {
                println!("{}", err);
                return;
            }
        };

        // Create all parent directories if they don't exist
        if let Some(parent) = Path::new(&rewards_file_path).parent() {
            match std::fs::create_dir_all(parent) {
                Ok(_) => (),
                Err(err) => {
                    println!(
                        "{}",
                        format!("Error: Failed to create directory - {}", err).red()
                    );
                    return;
                }
            };
        }

        match File::create(&rewards_file_path)
            .map_err(|e| e.to_string())
            .and_then(|file| {
                serde_json::to_writer_pretty(
                    file,
                    &json!({
                        "total_block_rewards": total_block_rewards,
                    }),
                )
                .map_err(|e| e.to_string())
            }) {
            Ok(_) => (),
            Err(err) => {
                println!("{}", format!("Error: {}", err).red());
                return;
            }
        };

        println!(
            "{}",
            format!(
                "✓ Total block rewards for {} in epoch {} are {} SOL",
                &identity_pubkey.to_string()[..6],
                epoch,
                TokenAmt {
                    amt: total_block_rewards,
                    decimals: 9
                }
            )
            .green()
            .bold()
        );

        println!(
            "{}",
            format!("Saved rewards to {}", rewards_file_path).blue()
        );

        println!("{}", "=".repeat(80));
    }
}
