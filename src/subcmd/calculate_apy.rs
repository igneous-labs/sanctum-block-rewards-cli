
use crate::{
    get_rewards_file_path, inflation::{get_real_apy, EpochBlockReward}, input_string,
    input_with_validation, subcmd::Subcmd, validate_epoch, SOLANA_PUBLIC_RPC,
};
use clap::Args;
use colored::Colorize;
use duners::{
    client::DuneClient,
    parameters::Parameter,
    response::{ExecutionResponse, ExecutionStatus, GetResultResponse, GetStatusResponse},
};
use inquire::Confirm;
use sanctum_solana_cli_utils::TokenAmt;
use serde_json;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use spinners::{Spinner, Spinners};
use std::{fs::File, path::Path, str::FromStr, time::Duration};

const DUNE_QUERY_ID: u32 = 6723900;
const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

#[derive(Args, Debug)]
#[command(
    long_about = "Calculate the real APY (inflation + block rewards) for your validator over a range of epochs."
)]
pub struct CalculateAPYArgs {
    #[arg(long, help = "The identity pubkey of your validator")]
    pub identity_pubkey: Option<String>,

    #[arg(long, help = "The vote account pubkey of your validator")]
    pub vote_account: Option<String>,

    #[arg(long, help = "Dune API key")]
    pub dune_api_key: Option<String>,

    #[arg(long, help = "Helius API key (for fetching inflation rewards)")]
    pub helius_api_key: Option<String>,

    #[arg(long, help = "The start epoch to calculate apy for")]
    pub start_epoch: Option<u64>,

    #[arg(long, help = "The end epoch to calculate apy for")]
    pub end_epoch: Option<u64>,

    #[arg(
        long,
        help = "Timeout in seconds for waiting for query results (default: 300)",
        default_value_t = DEFAULT_TIMEOUT_SECS
    )]
    pub timeout: u64,
}

impl CalculateAPYArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_pubkey,
            vote_account,
            dune_api_key,
            helius_api_key,
            start_epoch,
            end_epoch,
            timeout,
        } = match args.subcmd {
            Subcmd::CalculateAPY(args) => args,
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

        let vote_account = match input_string(
            "Enter your validator's vote account:",
            "Vote account",
            None,
            vote_account,
        ) {
            Ok(key) => key,
            Err(_) => {
                println!("{}", "Error: Invalid vote account".red());
                return;
            }
        };

        // Validate vote account is a valid pubkey
        if Pubkey::from_str(&vote_account).is_err() {
            println!("{}", "Error: Invalid vote account pubkey".red());
            return;
        }

        let rpc = RpcClient::new_with_commitment(
            SOLANA_PUBLIC_RPC.to_string(),
            args.commitment.unwrap_or(CommitmentConfig::confirmed()),
        );

        let current_epoch_info = match rpc.get_epoch_info().await {
            Ok(info) => info,
            Err(_) => {
                println!("{}", "Error: Failed to get current epoch info".red());
                return;
            }
        };

        let start_epoch = match input_with_validation(
            "Enter the start epoch:",
            &(current_epoch_info.epoch - 1).to_string(),
            Some((current_epoch_info.epoch - 1).to_string()),
            start_epoch.map(|e| e.to_string()),
            |input| validate_epoch(input, current_epoch_info.epoch),
        ) {
            Ok(e) => e,
            Err(_) => {
                println!("{}", "Error: Invalid epoch".red());
                return;
            }
        };

        let end_epoch = match input_with_validation(
            "Enter the end epoch:",
            &(current_epoch_info.epoch - 1).to_string(),
            Some((current_epoch_info.epoch - 1).to_string()),
            end_epoch.map(|e| e.to_string()),
            |input| validate_epoch(input, current_epoch_info.epoch),
        ) {
            Ok(e) => e,
            Err(_) => {
                println!("{}", "Error: Invalid epoch".red());
                return;
            }
        };

        let dune_api_key =
            match input_string("Enter your Dune API key:", "API key", None, dune_api_key) {
                Ok(key) => key,
                Err(_) => {
                    println!("{}", "Error: Invalid Dune API key".red());
                    return;
                }
            };

        let helius_api_key =
            match input_string("Enter your Helius API key:", "API key", None, helius_api_key) {
                Ok(key) => key,
                Err(_) => {
                    println!("{}", "Error: Invalid Helius API key".red());
                    return;
                }
            };

        let helius_rpc_url = format!("https://mainnet.helius-rpc.com/?api-key={}", helius_api_key);

        println!("{}", "=".repeat(80));

        // Check if rewards file exists
        let rewards_file_path = match get_rewards_file_path(&identity_pubkey, start_epoch, end_epoch) {
            Ok(path) => path,
            Err(err) => {
                println!("{}", format!("Error: {}", err).red());
                return;
            }
        };

        // Try to load cached per-epoch block rewards
        let epoch_block_rewards: Vec<EpochBlockReward> = if Path::new(&rewards_file_path).exists()
        {
            #[derive(serde::Deserialize)]
            struct CachedReward {
                epoch: u64,
                block_rewards: u64,
            }

            let cached: Option<Vec<CachedReward>> = File::open(&rewards_file_path)
                .ok()
                .and_then(|file| serde_json::from_reader(file).ok());

            if let Some(cached) = cached {
                println!(
                    "{}",
                    format!("Rewards file found at {}", rewards_file_path).blue()
                );
                cached.into_iter().map(|c| (c.epoch, c.block_rewards)).collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let epoch_block_rewards = if epoch_block_rewards.is_empty() {
            println!(
                "{}",
                "⚠️ Please note that the data on Dune is usually lagging by 2-3 hours"
                    .bold()
                    .yellow()
            );

            let ans = Confirm::new(
                &"Do you wish to continue with fetching block rewards?"
                    .blue()
                    .bold(),
            )
            .with_default(true)
            .prompt();

            match ans {
                Ok(false) => return,
                Err(_) => {
                    println!("Error: Something went wrong.");
                    return;
                }
                _ => (),
            }

            println!("{}", "=".repeat(80));

            let mut sp = Spinner::new(
                Spinners::Dots,
                format!(
                    "Executing Dune query for {}...",
                    &identity_pubkey.to_string()[..6]
                ),
            );

            let dune_client = DuneClient::new(&dune_api_key);

            let ExecutionResponse { execution_id, .. } = match dune_client
                .execute_query(
                    DUNE_QUERY_ID,
                    Some(vec![
                        Parameter::number("start_epoch", &start_epoch.to_string()),
                        Parameter::number("end_epoch", &end_epoch.to_string()),
                        Parameter::text("identity_pubkey", &identity_pubkey.to_string()),
                    ]),
                )
                .await
            {
                Ok(response) => response,
                Err(_) => {
                    sp.stop_with_message("Error: Failed to execute query".red().to_string());
                    return;
                }
            };

            sp.stop();
            let mut sp = Spinner::new(
                Spinners::Dots,
                format!("Waiting for result of execution ID: {}", execution_id),
            );

            // Poll for results
            let mut result_rows: Option<Vec<EpochBlockReward>> = None;
            let poll_interval_secs = 5;
            let max_attempts = timeout / poll_interval_secs;

            for _ in 0..max_attempts {
                let GetStatusResponse { state, .. } =
                    match dune_client.get_status(&execution_id).await {
                        Ok(status) => status,
                        Err(_) => {
                            sp.stop_with_message(
                                "Error: Failed to get execution status".red().to_string(),
                            );
                            return;
                        }
                    };

                match state {
                    ExecutionStatus::Failed => {
                        sp.stop_with_message(
                            "Error: Query execution failed".red().to_string(),
                        );
                        return;
                    }
                    ExecutionStatus::Cancelled => {
                        sp.stop_with_message(
                            "Error: Query execution cancelled".red().to_string(),
                        );
                        return;
                    }
                    ExecutionStatus::Complete => {
                        #[derive(Debug, serde::Deserialize)]
                        struct ResultStruct {
                            epoch: u64,
                            block_rewards: u64,
                        }

                        let GetResultResponse::<ResultStruct> { result, .. } =
                            match dune_client.get_results::<ResultStruct>(&execution_id).await {
                                Ok(r) => r,
                                Err(_) => {
                                    sp.stop_with_message(
                                        "Error: Failed to get execution results"
                                            .red()
                                            .to_string(),
                                    );
                                    return;
                                }
                            };

                        if result.rows.is_empty() {
                            sp.stop_with_message(
                                format!(
                                    "Error: No rewards data found for epoch {}~{}",
                                    start_epoch, end_epoch
                                )
                                .red()
                                .to_string(),
                            );
                            return;
                        }

                        result_rows = Some(
                            result
                                .rows
                                .into_iter()
                                .map(|r| (r.epoch, r.block_rewards))
                                .collect(),
                        );
                        break;
                    }
                    _ => {
                        tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
                        continue;
                    }
                }
            }

            let epoch_block_rewards = match result_rows {
                Some(rows) => rows,
                None => {
                    sp.stop_with_message("Error: Query timed out".red().to_string());
                    return;
                }
            };

            sp.stop_with_message(
                "✓ Execution completed!"
                    .to_string()
                    .green()
                    .bold()
                    .to_string(),
            );

            println!("{}", "=".repeat(80));

            // Cache per-epoch block rewards to file
            if let Some(parent) = Path::new(&rewards_file_path).parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    println!(
                        "{}",
                        format!("Error: Failed to create directory - {}", err).red()
                    );
                    return;
                }
            }

            #[derive(serde::Serialize)]
            struct CachedReward {
                epoch: u64,
                block_rewards: u64,
            }

            let cached: Vec<CachedReward> = epoch_block_rewards
                .iter()
                .map(|(epoch, block_rewards)| CachedReward {
                    epoch: *epoch,
                    block_rewards: *block_rewards,
                })
                .collect();

            match File::create(&rewards_file_path)
                .map_err(|e| e.to_string())
                .and_then(|file| {
                    serde_json::to_writer_pretty(file, &cached).map_err(|e| e.to_string())
                }) {
                Ok(_) => {
                    println!(
                        "{}",
                        format!("Saved block rewards to {}", rewards_file_path).blue()
                    );
                }
                Err(err) => {
                    println!("{}", format!("Error: {}", err).red());
                    return;
                }
            };

            epoch_block_rewards
        } else {
            epoch_block_rewards
        };

        // Display per-epoch block rewards summary
        let total_lamports: u64 = epoch_block_rewards.iter().map(|(_, r)| r).sum();
        println!(
            "{}",
            format!(
                "✓ Total block rewards for {}... in epoch {}~{}: {} SOL",
                &identity_pubkey.to_string()[..6],
                start_epoch,
                end_epoch,
                TokenAmt {
                    amt: total_lamports,
                    decimals: 9
                }
            )
            .green()
            .bold()
        );

        println!("{}", "=".repeat(80));

        // Calculate real APY (inflation + block rewards)
        let mut sp = Spinner::new(
            Spinners::Dots,
            "Calculating real APY (inflation + block rewards)...".to_string(),
        );

        match get_real_apy(&vote_account, epoch_block_rewards, current_epoch_info.epoch, &helius_rpc_url).await {
            Ok(apy) => {
                sp.stop_with_message(
                    format!("✓ Real APY: {:.4}%", apy * 100.0)
                        .green()
                        .bold()
                        .to_string(),
                );
            }
            Err(err) => {
                sp.stop_with_message(
                    format!("Error calculating APY: {}", err).red().to_string(),
                );
            }
        }

        println!("{}", "=".repeat(80));
    }
}
