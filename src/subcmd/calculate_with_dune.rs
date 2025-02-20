use crate::{
    get_rewards_file_path, input_string, input_with_validation, subcmd::Subcmd, validate_epoch,
    SOLANA_PUBLIC_RPC,
};
use clap::{command, Args};
use colored::Colorize;
use duners::{
    client::DuneClient,
    parameters::Parameter,
    response::{ExecutionResponse, ExecutionStatus, GetResultResponse, GetStatusResponse},
};
use inquire::Confirm;
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner, TokenAmt};
use serde_json::{json, Value};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use spinners::{Spinner, Spinners};
use std::{fs::File, path::Path, time::Duration};

const DUNE_QUERY_ID: u32 = 4745888;
// const DUNE_QUERY_ID: u32 = 4750136;
const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

#[derive(Args, Debug)]
#[command(
    long_about = "Calculate the total block rewards earned by your validator for a specific epoch."
)]
pub struct CalculateWithDuneArgs {
    #[arg(long, help = "The identity keypair of your validator")]
    pub identity_keypair_path: String,

    #[arg(long, help = "Dune API key")]
    pub dune_api_key: Option<String>,

    #[arg(long, help = "The epoch to calculate rewards for")]
    pub epoch: Option<u64>,

    #[arg(
        long,
        help = "Timeout in seconds for waiting for query results (default: 300)",
        default_value_t = DEFAULT_TIMEOUT_SECS
    )]
    pub timeout: u64,
}

impl CalculateWithDuneArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_keypair_path,
            dune_api_key,
            epoch,
            timeout,
        } = match args.subcmd {
            Subcmd::CalculateWithDune(args) => args,
            _ => unreachable!(),
        };

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

        let dune_api_key =
            match input_string("Enter your Dune API key:", "API key", None, dune_api_key) {
                Ok(key) => key,
                Err(_) => {
                    println!("{}", "Error: Invalid Dune API key".red());
                    return;
                }
            };

        println!("{}", "=".repeat(80));

        let identity_keypair = match parse_named_signer(ParseNamedSigner {
            name: "identity",
            arg: &identity_keypair_path,
        }) {
            Ok(keypair) => keypair,
            Err(_) => {
                println!("{}", "Error: Invalid identity keypair".red());
                return;
            }
        };

        let identity_pubkey = identity_keypair.pubkey();

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
                    Parameter::number("epoch", &epoch.to_string()),
                    Parameter::text(
                        "identity_pubkey",
                        "JupRhwjrF5fAcs6dFhLH59r3TJFvbcyLP2NRM8UGH9H",
                    ),
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

        // Update spinner message with execution ID
        sp.stop();
        let mut sp = Spinner::new(
            Spinners::Dots,
            format!("Waiting for result of execution ID: {}", execution_id),
        );

        // Poll for results
        let mut total_block_rewards = None;
        let poll_interval_secs = 5;
        let max_attempts = timeout / poll_interval_secs;

        for _ in 0..max_attempts {
            // Poll until timeout

            let GetStatusResponse { state, .. } = match dune_client.get_status(&execution_id).await
            {
                Ok(status) => status,
                Err(_) => {
                    sp.stop_with_message("Error: Failed to get execution status".red().to_string());
                    return;
                }
            };

            match state {
                ExecutionStatus::Failed => {
                    sp.stop_with_message("Error: Query execution failed".red().to_string());
                    return;
                }
                ExecutionStatus::Cancelled => {
                    sp.stop_with_message("Error: Query execution cancelled".red().to_string());
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
                                    "Error: Failed to get execution results".red().to_string(),
                                );
                                return;
                            }
                        };

                    for row in result.rows {
                        if row.epoch == epoch {
                            total_block_rewards = Some(row.block_rewards);
                            break;
                        }
                    }

                    if total_block_rewards.is_some() {
                        break;
                    }

                    sp.stop_with_message(
                        format!("Error: No rewards data found for epoch {}", epoch)
                            .red()
                            .to_string(),
                    );
                    return;
                }
                _ => {
                    tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
                    continue;
                }
            }
        }

        let total_block_rewards = match total_block_rewards {
            Some(rewards) => rewards,
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

        // Save results to file
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

        println!(
            "{}",
            format!("Saved rewards to {}", rewards_file_path).blue()
        );

        println!("{}", "=".repeat(80));
    }
}
