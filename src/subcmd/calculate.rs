use std::{fs::File, path::Path};

use crate::{
    get_leader_slots_for_identity, get_rewards_file_path, get_total_block_rewards_for_slots,
    input_with_validation, lamports_to_pretty_sol, subcmd::Subcmd, validate_epoch,
    validate_rpc_url, SOLANA_PUBLIC_RPC,
};
use clap::{command, Args};
use colored::Colorize;
use inquire::Confirm;
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner};
use serde_json::{json, Value};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use spinners::{Spinner, Spinners};
use tokio;

#[derive(Args, Debug)]
#[command(
    long_about = "Calculate the total block rewards earned by your validator for a specific epoch."
)]
pub struct CalculateArgs {
    #[arg(long, help = "The identity keypair of your validator")]
    pub identity_keypair_path: String,
    #[arg(long, help = "The epoch to calculate rewards for")]
    pub epoch: Option<u64>,
}

impl CalculateArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_keypair_path,
            epoch,
        } = match args.subcmd {
            Subcmd::Calculate(args) => args,
            _ => unreachable!(),
        };

        let rpc_url_result = input_with_validation(
            "Enter the RPC URL:",
            "RPC URL",
            Some(SOLANA_PUBLIC_RPC.to_string()),
            args.rpc_url,
            |input| validate_rpc_url(input),
        );
        if rpc_url_result.is_err() {
            println!("{}", format!("Error: Invalid RPC URL").red());
            return;
        }
        let rpc_url = rpc_url_result.unwrap();

        let rpc = RpcClient::new_with_commitment(
            rpc_url,
            args.commitment.unwrap_or(CommitmentConfig::confirmed()),
        );

        let rpc_call_result = tokio::try_join!(rpc.get_epoch_info(), rpc.get_epoch_schedule());

        if rpc_call_result.is_err() {
            println!("{}", format!("Error: Failed to fetch data from RPC").red());
            return;
        }

        let (current_epoch_info, epoch_schedule) = rpc_call_result.unwrap();

        let epoch_result = input_with_validation(
            "Enter the epoch to calculate rewards for:",
            &(current_epoch_info.epoch - 1).to_string(),
            Some((current_epoch_info.epoch - 1).to_string()),
            epoch.map(|e| e.to_string()),
            |input| validate_epoch(input, current_epoch_info.epoch),
        );
        if epoch_result.is_err() {
            println!("{}", format!("Error: Invalid epoch").red());
            return;
        }
        let epoch = epoch_result.unwrap();

        println!("{}", "=".repeat(80));

        let identity_keypair = parse_named_signer(ParseNamedSigner {
            name: "identity",
            arg: &identity_keypair_path,
        });

        if identity_keypair.is_err() {
            println!("{}", format!("Error: Invalid identity keypair").red());
            return;
        }

        let identity_keypair = identity_keypair.unwrap();

        let identity_pubkey = identity_keypair.pubkey();

        // Check if rewards file exists
        let rewards_file_path = get_rewards_file_path(&identity_pubkey, epoch);

        // if path exists, read the file and display the total block rewards
        if Path::new(&rewards_file_path).exists() {
            let rewards_file = File::open(rewards_file_path.clone()).unwrap();
            let rewards: Value = serde_json::from_reader(rewards_file).unwrap();
            let total_block_rewards_sol =
                lamports_to_pretty_sol(rewards["total_block_rewards"].as_u64().unwrap());

            println!(
                "{}",
                format!("Rewards file found at {}", rewards_file_path)
                    .blue()
                    .to_string()
            );
            println!(
                "{}",
                format!(
                    "✓ Total block rewards for {}... in epoch {} are {} SOL",
                    identity_pubkey.to_string()[..6].to_string(),
                    epoch,
                    total_block_rewards_sol
                )
                .green()
                .bold()
                .to_string()
            );

            println!("{}", "=".repeat(80));
            return;
        }

        let mut sp = Spinner::new(
            Spinners::Dots,
            format!(
                "Fetching leader slots for {}...",
                identity_pubkey.to_string()[..6].to_string()
            ),
        );

        let leader_slots =
            get_leader_slots_for_identity(&rpc, epoch, &epoch_schedule, &identity_pubkey).await;

        if leader_slots.is_err() {
            println!(
                "{}",
                format!("Error: {}", leader_slots.err().unwrap()).red()
            );
            return;
        }

        let leader_slots = leader_slots.unwrap();

        let num_leader_slots = leader_slots.len();
        sp.stop_with_message(
            format!(
                "✓ Found {} leader slots for {}... in epoch {}",
                num_leader_slots,
                identity_pubkey.to_string()[..6].to_string(),
                epoch
            )
            .green()
            .bold()
            .to_string(),
        );

        if leader_slots.len() > 200 && rpc.url() == SOLANA_PUBLIC_RPC {
            println!(
                "{}",
                format!("⚠️ We recommend using a custom RPC URL to avoid longer wait time and rate limits.",)
                    .yellow()
                    .bold()
            );
        }

        println!("{}", "=".repeat(80));

        let ans = Confirm::new(
            &"Do you wish to continue with fetching block rewards?"
                .blue()
                .bold()
                .to_string(),
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

        let total_block_rewards = get_total_block_rewards_for_slots(&rpc, &leader_slots).await;
        if total_block_rewards.is_err() {
            println!("{}", total_block_rewards.err().unwrap());
            return;
        }

        let total_block_rewards = total_block_rewards.unwrap();
        let total_block_rewards_sol = lamports_to_pretty_sol(total_block_rewards);

        // Create all parent directories if they don't exist
        if let Some(parent) = Path::new(&rewards_file_path).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }

        let rewards_file = File::create(rewards_file_path.clone()).unwrap();
        serde_json::to_writer_pretty(
            rewards_file,
            &json!({
                "total_block_rewards": total_block_rewards,
            }),
        )
        .unwrap();

        println!(
            "{}",
            format!(
                "✓ Total block rewards for {} in epoch {} are {} SOL",
                identity_pubkey.to_string()[..6].to_string(),
                epoch,
                total_block_rewards_sol
            )
            .green()
            .bold()
            .to_string()
        );

        println!(
            "{}",
            format!("Saved rewards to {}", rewards_file_path)
                .blue()
                .to_string()
        );

        println!("{}", "=".repeat(80));
    }
}
