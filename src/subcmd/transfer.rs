use crate::{
    checked_pct, get_lst_info, get_rewards_file_path, handle_tx_full, input_with_validation,
    print_transfer_summary, subcmd::Subcmd, transfer_to_reserve_and_update_stake_pool_balance_ixs,
    validate_bps, validate_epoch, validate_pubkey, validate_rpc_url, with_auto_cb_ixs,
    PrintTransferSummaryArgs, SOLANA_PUBLIC_RPC,
};
use clap::{command, Args};
use colored::Colorize;
use inquire::Confirm;
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner, TxSendMode};
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::{fs::File, path::Path};

#[derive(Args, Debug)]
#[command(long_about = "Transfer block rewards to the stake pool reserve")]
pub struct TransferArgs {
    #[arg(long, help = "The identity keypair for your validator")]
    pub identity_keypair_path: String,

    #[arg(long, help = "The epoch to calculate rewards for")]
    pub epoch: Option<u64>,

    #[arg(long, help = "The stake pool account linked to your LST")]
    pub stake_pool_pubkey: Option<String>,

    #[arg(
        long,
        help = "Percentage of stake you want to consider for calculating the block rewards"
    )]
    pub total_rewards_pct: Option<u64>,

    #[arg(long, help = "Percentage of block rewards to share to LST holders")]
    pub lst_rewards_pct: Option<u64>,
}

impl TransferArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_keypair_path,
            epoch,
            stake_pool_pubkey,
            total_rewards_pct,
            lst_rewards_pct,
        } = match args.subcmd {
            Subcmd::Transfer(a) => a,
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
            println!("{}", "Error: Invalid RPC URL".red());
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
            println!("{}", "Error: Invalid identity keypair".red());
            return;
        }

        let identity_keypair = identity_keypair.unwrap();

        let identity_pubkey = identity_keypair.pubkey();

        let rpc_call_result =
            tokio::try_join!(rpc.get_epoch_info(), rpc.get_balance(&identity_pubkey));

        if rpc_call_result.is_err() {
            println!("{}", "Error: Failed to fetch data from RPC".red());
            return;
        }

        let (current_epoch_info, identity_balance) = rpc_call_result.unwrap();

        let epoch_result = input_with_validation(
            "Enter the epoch to calculate rewards for:",
            &(current_epoch_info.epoch - 1).to_string(),
            Some((current_epoch_info.epoch - 1).to_string()),
            epoch.map(|e| e.to_string()),
            |input| validate_epoch(input, current_epoch_info.epoch),
        );
        if epoch_result.is_err() {
            println!("{}", "Error: Invalid epoch".red());
            return;
        }
        let epoch = epoch_result.unwrap();

        println!("{}", "=".repeat(80));

        let rewards_file_path = get_rewards_file_path(&identity_pubkey, epoch);

        if Path::new(&rewards_file_path).exists() == false {
            println!(
                "{}",
                format!("Failed to find rewards at {}", rewards_file_path).blue()
            );
            println!(
                "{}",
                "Please run the calculate command first to generate the rewards file."
                    .blue()
                    .bold()
            );

            println!("{}", "=".repeat(80));
            return;
        }

        let rewards_file = File::open(rewards_file_path.clone()).unwrap();
        let rewards: Value = serde_json::from_reader(rewards_file).unwrap();

        let total_block_rewards = rewards["total_block_rewards"].as_u64().unwrap();

        let stake_pool_pubkey_result = input_with_validation(
            "Enter the stake pool pubkey:",
            "Stake pool pubkey",
            None,
            stake_pool_pubkey,
            |input| validate_pubkey(input),
        );
        if stake_pool_pubkey_result.is_err() {
            println!("{}", "Error: Invalid pubkey".red());
            return;
        }

        let stake_pool_pubkey = stake_pool_pubkey_result.unwrap();

        let lst_info = get_lst_info(&stake_pool_pubkey.to_string()).await;
        if lst_info.is_err() {
            println!(
                "{}",
                "⚠ We could not find a LST for the specified address".yellow()
            );
            return;
        }

        let (lst_name, lst_symbol) = lst_info.unwrap();

        let total_rewards_bps_result = input_with_validation(
            "Enter the percentage of stake you want to consider for calculating the block rewards:",
            "75",
            None,
            total_rewards_pct.map(|bps| bps.to_string()),
            validate_bps,
        );
        if total_rewards_bps_result.is_err() {
            println!("{}", "Error: Invalid total rewards BPS".red());
            return;
        }
        let total_rewards_bps = total_rewards_bps_result.unwrap();

        let lst_rewards_bps_result = input_with_validation(
            "Enter the percentage of block rewards to share:",
            "100",
            None,
            lst_rewards_pct.map(|bps| bps.to_string()),
            validate_bps,
        );
        if lst_rewards_bps_result.is_err() {
            println!("{}", "Error: Invalid LST rewards BPS".red());
            return;
        }
        let lst_rewards_bps = lst_rewards_bps_result.unwrap();

        // Calculate stake pool's share
        let stake_pool_rewards = match checked_pct(total_block_rewards, total_rewards_bps) {
            Some(rewards) => rewards,
            None => {
                println!("{}", "Error: Error in calculating stake pool rewards".red());
                return;
            }
        };

        // Calculate LST holders' share
        let lst_rewards = match checked_pct(stake_pool_rewards, lst_rewards_bps) {
            Some(rewards) => rewards,
            None => {
                println!("{}", "Error: Overflow in calculating LST rewards".red());
                return;
            }
        };

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

        println!("{}", "=".repeat(80));

        let confirm_message = format!(
            "Do you wish to continue to transfer your block rewards to {} ({})?",
            lst_name.magenta(),
            lst_symbol.magenta()
        );

        let ans = Confirm::new(&confirm_message.blue().bold())
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

        if final_ixs.is_err() {
            println!("{}", final_ixs.err().unwrap());
            return;
        }

        let final_ixs = final_ixs.unwrap();

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
