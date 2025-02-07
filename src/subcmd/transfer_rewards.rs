use std::fmt::Write;

use crate::subcmd::Subcmd;
use clap::{command, Args};
use futures;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner};
use solana_client::rpc_config::{RpcBlockConfig, RpcLeaderScheduleConfig};
use solana_sdk::commitment_config::CommitmentConfig;
use tokio;

const MINIMUM_SLOTS_PER_EPOCH: u64 = 32;

#[derive(Args, Debug)]
#[command(long_about = "Deposit an activated stake account into a stake pool")]
pub struct TransferRewardsArgs {
    #[arg(long, short, help = "The identity keypair for your validator")]
    pub identity_keypair_path: String,
    // #[arg(long, short, help = "The stake pool account linked to your LST")]
    // pub stake_pool_account: String,

    // #[arg(
    //     long,
    //     short,
    //     help = "The percentage of total rewards to consider as rewards for the stake pool (in basis points)"
    // )]
    // pub total_rewards_pct: u64,

    // #[arg(
    //     long,
    //     short,
    //     help = "The percentage of stake pool rewards to distribute among LST holders (in basis points)"
    // )]
    // pub lst_rewards_pct: u64,
}

impl TransferRewardsArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_keypair_path,
            // stake_pool_account,
            // total_rewards_pct,
            // lst_rewards_pct,
        } = match args.subcmd {
            Subcmd::TransferRewards(a) => a,
        };

        let rpc = args.config.nonblocking_rpc_client();

        let (current_epoch_info, epoch_schedule) =
            tokio::try_join!(rpc.get_epoch_info(), rpc.get_epoch_schedule()).unwrap();

        let identity_keypair = parse_named_signer(ParseNamedSigner {
            name: "identity",
            arg: &identity_keypair_path,
        })
        .unwrap();

        let _identity_pubkey = identity_keypair.pubkey().to_string();

        // Calculate the first slot of the previous epoch
        // Reference: https://github.com/solana-foundation/explorer/blob/ad529a6b9692be98096c55459e6406c0dd1654c5/app/utils/epoch-schedule.ts#L63
        let previous_epoch = current_epoch_info.epoch - 1;
        let previous_epoch_first_slot = if previous_epoch <= epoch_schedule.first_normal_epoch {
            (1u64 << previous_epoch) * MINIMUM_SLOTS_PER_EPOCH
        } else {
            (previous_epoch - epoch_schedule.first_normal_epoch) * epoch_schedule.slots_per_epoch
                + epoch_schedule.first_normal_slot
        };

        let previous_epoch_leader_schedule = rpc
            .get_leader_schedule_with_config(
                Some(previous_epoch_first_slot),
                RpcLeaderScheduleConfig {
                    identity: Some("SDEVqCDyc3YzjrDn375SMWKpZo1m7tbZ12fsenF48x1".to_string()), // TODO(sk): Replace with identity_pubkey
                    commitment: None,
                },
            )
            .await
            .unwrap();

        if previous_epoch_leader_schedule.is_none() {
            println!("Validator not found in leader schedule for previous epoch");
            return;
        }

        let relative_leader_slots = previous_epoch_leader_schedule
            .unwrap()
            .get("SDEVqCDyc3YzjrDn375SMWKpZo1m7tbZ12fsenF48x1")
            .unwrap_or(&vec![])
            .to_vec();

        let num_leader_slots: u64 = relative_leader_slots.len().try_into().unwrap();

        println!("Found {} leader slots in previous epoch", num_leader_slots);
        if num_leader_slots == 0 {
            println!("No leader slots found for the validator in previous epoch");
            return;
        }

        println!("Fetching block rewards...");

        // Setup progress bar
        let pb = ProgressBar::new(num_leader_slots);
        pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} slots ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));

        let mut reward_lamports: u64 = 0;

        for (i, &leader_slot) in relative_leader_slots.iter().enumerate() {
            let absolute_slot = previous_epoch_first_slot + leader_slot as u64;

            let block = rpc
                .get_block_with_config(
                    absolute_slot,
                    RpcBlockConfig {
                        rewards: Some(true),
                        commitment: Some(CommitmentConfig::confirmed()),
                        max_supported_transaction_version: Some(0),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();

            if let Some(rewards) = block.rewards {
                for reward in rewards {
                    reward_lamports += reward.lamports as u64;
                }
            }

            pb.inc(1);
        }

        pb.finish_with_message("Done fetching rewards");
        println!("\nTotal reward lamports: {}", reward_lamports);
    }
}
