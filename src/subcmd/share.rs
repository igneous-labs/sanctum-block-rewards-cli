use crate::{
    get_leader_slots_for_identity, get_rewards_file_path, get_total_block_rewards_for_slots,
    handle_tx_full, input_with_validation, print_transfer_summary, subcmd::Subcmd,
    transfer_to_reserve_and_update_stake_pool_balance_ixs, with_auto_cb_ixs,
    PrintTransferSummaryArgs, SOLANA_PUBLIC_RPC,
};
use crate::{
    lamports_to_pretty_sol, validate_bps, validate_epoch, validate_pubkey, validate_rpc_url,
};
use clap::{command, Args};
use colored::Colorize;
use inquire::Confirm;
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner, TxSendMode};
use serde_json::{json, Value};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use spinners::{Spinner, Spinners};
use std::{fs::File, path::Path};
use tokio;

#[derive(Args, Debug)]
#[command(long_about = "Deposit an activated stake account into a stake pool")]
pub struct ShareArgs {
    #[arg(long, short, help = "The identity keypair for your validator")]
    pub identity_keypair_path: String,

    #[arg(long, help = "The epoch to calculate rewards for")]
    pub epoch: Option<u64>,

    #[arg(long, short, help = "The stake pool account linked to your LST")]
    pub stake_pool_pubkey: Option<String>,

    #[arg(
        long,
        short,
        help = "The percentage (in basis points) of total rewards to consider as rewards for the stake pool "
    )]
    pub total_rewards_bps: Option<u64>,

    #[arg(
        long,
        short,
        help = "The percentage (in basis points) of stake pool rewards to distribute among LST holders"
    )]
    pub lst_rewards_bps: Option<u64>,
}

impl ShareArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_keypair_path,
            epoch,
            stake_pool_pubkey,
            total_rewards_bps,
            lst_rewards_bps,
        } = match args.subcmd {
            Subcmd::Share(a) => a,
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

        let rpc_call_result = tokio::try_join!(
            rpc.get_epoch_info(),
            rpc.get_epoch_schedule(),
            rpc.get_balance(&identity_pubkey)
        );

        if rpc_call_result.is_err() {
            println!("{}", format!("Error: Failed to fetch data from RPC").red());
            return;
        }

        let (current_epoch_info, epoch_schedule, identity_balance) = rpc_call_result.unwrap();

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

        let rewards_file_path = get_rewards_file_path(&identity_pubkey, epoch);

        let total_block_rewards = if Path::new(&rewards_file_path).exists() {
            let rewards_file = File::open(rewards_file_path.clone()).unwrap();
            let rewards: Value = serde_json::from_reader(rewards_file).unwrap();
            let total_block_rewards = rewards["total_block_rewards"].as_u64().unwrap();

            total_block_rewards
        } else {
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

            total_block_rewards
        };

        let stake_pool_pubkey_result = input_with_validation(
            "Enter the stake pool pubkey:",
            "Stake pool pubkey",
            None,
            stake_pool_pubkey,
            |input| validate_pubkey(input),
        );
        if stake_pool_pubkey_result.is_err() {
            println!("{}", format!("Error: Invalid pubkey").red());
            return;
        }

        let stake_pool_pubkey = stake_pool_pubkey_result.unwrap();

        let total_rewards_bps_result = input_with_validation(
            "Enter the percentage of LST-allocated stake:",
            "75",
            None,
            total_rewards_bps.map(|bps| bps.to_string()),
            validate_bps,
        );
        if total_rewards_bps_result.is_err() {
            println!("{}", format!("Error: Invalid total rewards BPS").red());
            return;
        }
        let total_rewards_bps = total_rewards_bps_result.unwrap();

        let lst_rewards_bps_result = input_with_validation(
            "Enter the percentage of block rewards to share:",
            "100",
            None,
            lst_rewards_bps.map(|bps| bps.to_string()),
            validate_bps,
        );
        if lst_rewards_bps_result.is_err() {
            println!("{}", format!("Error: Invalid LST rewards BPS").red());
            return;
        }
        let lst_rewards_bps = lst_rewards_bps_result.unwrap();

        // Calculate stake pool's share (total_rewards_bps is in basis points - 1/100th of a percent)
        let stake_pool_rewards = (total_block_rewards as u64 * total_rewards_bps as u64) / 10_000;

        // Calculate LST holders' share
        let lst_rewards = (stake_pool_rewards * lst_rewards_bps as u64) / 10_000;

        println!("{}", "=".repeat(80));

        print_transfer_summary(PrintTransferSummaryArgs {
            epoch,
            identity_balance,
            total_block_rewards,
            total_rewards_bps,
            stake_pool_rewards,
            lst_rewards_bps,
            lst_rewards,
        });

        let ans = Confirm::new(
            &"Do you wish to continue with the transfer?"
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

        let send_mode = args.send_mode;
        let fee_limit_cb = args.fee_limit_cb;

        let final_ixs = transfer_to_reserve_and_update_stake_pool_balance_ixs(
            &rpc,
            &identity_pubkey,
            &stake_pool_pubkey,
            lst_rewards,
        )
        .await;

        let final_ixs = match send_mode {
            TxSendMode::DumpMsg => final_ixs,
            _ => with_auto_cb_ixs(&rpc, &identity_pubkey, final_ixs, &[], fee_limit_cb).await,
        };

        if send_mode == TxSendMode::DumpMsg {
            println!("{}", "Transaction Message:".blue().bold());
        }

        handle_tx_full(&rpc, send_mode, &final_ixs, &[], &mut [&identity_keypair]).await;
    }
}
