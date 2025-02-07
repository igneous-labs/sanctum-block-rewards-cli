use borsh::BorshDeserialize;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use sanctum_spl_stake_pool_lib::{deserialize_stake_pool_checked, FindWithdrawAuthority};
use spl_stake_pool_interface::{
    update_stake_pool_balance_ix_with_program_id, StakePool, UpdateStakePoolBalanceKeys,
};
use std::fmt::Write;
use std::sync::Arc;

use crate::{handle_tx_full, subcmd::Subcmd, with_auto_cb_ixs};
use clap::{command, Args};
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner, PubkeySrc, TxSendMode};
use solana_client::rpc_config::{RpcBlockConfig, RpcLeaderScheduleConfig};
use solana_sdk::{
    account::{Account, ReadableAccount},
    commitment_config::CommitmentConfig,
    system_instruction::transfer,
};
use tokio;

const MINIMUM_SLOTS_PER_EPOCH: u64 = 32;
const SLOT_CHUNK_SIZE: usize = 50;

#[derive(Args, Debug)]
#[command(long_about = "Deposit an activated stake account into a stake pool")]
pub struct TransferRewardsArgs {
    #[arg(long, short, help = "The identity keypair for your validator")]
    pub identity_keypair_path: String,

    #[arg(long, short, help = "The stake pool account linked to your LST")]
    pub stake_pool_pubkey: String,
    #[arg(
        long,
        short,
        help = "The percentage of total rewards to consider as rewards for the stake pool (in basis points)"
    )]
    pub total_rewards_pct: u64,

    #[arg(
        long,
        short,
        help = "The percentage of stake pool rewards to distribute among LST holders (in basis points)"
    )]
    pub lst_rewards_pct: u64,
}

impl TransferRewardsArgs {
    pub async fn run(args: crate::Args) {
        let Self {
            identity_keypair_path,
            stake_pool_pubkey,
            total_rewards_pct,
            lst_rewards_pct,
        } = match args.subcmd {
            Subcmd::TransferRewards(a) => a,
        };

        let rpc = args.config.nonblocking_rpc_client();
        let send_mode = args.send_mode;
        let fee_limit_cb = args.fee_limit_cb;

        // identity keypair, which will also be used as payer
        let identity_keypair = parse_named_signer(ParseNamedSigner {
            name: "identity",
            arg: &identity_keypair_path,
        })
        .unwrap();

        let identity_pubkey = identity_keypair.pubkey();

        // Get current epoch info and epoch schedule
        let (current_epoch_info, epoch_schedule) =
            tokio::try_join!(rpc.get_epoch_info(), rpc.get_epoch_schedule()).unwrap();

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
                    identity: Some("JupRhwjrF5fAcs6dFhLH59r3TJFvbcyLP2NRM8UGH9H".to_string()), // TODO(sk): Remove hard coded identity
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
            .get("JupRhwjrF5fAcs6dFhLH59r3TJFvbcyLP2NRM8UGH9H") // TODO(sk): Remove hard coded identity
            .unwrap_or(&vec![])
            .to_vec();

        let num_leader_slots: u64 = relative_leader_slots.len().try_into().unwrap();

        println!("Found {} leader slots in previous epoch", num_leader_slots);
        if num_leader_slots == 0 {
            println!("No leader slots found for the validator in previous epoch");
            return;
        }

        println!("Fetching block rewards...");

        let pb = ProgressBar::new(num_leader_slots);
        pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} slots ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));

        let mut total_rewards = 0u64;
        let mut slot_counter = 0;

        for &leader_slot in relative_leader_slots.iter() {
            let absolute_slot = previous_epoch_first_slot + leader_slot as u64;

            if let Ok(block) = rpc
                .get_block_with_config(
                    absolute_slot,
                    RpcBlockConfig {
                        rewards: Some(true),
                        commitment: Some(CommitmentConfig::confirmed()),
                        max_supported_transaction_version: Some(0),
                        transaction_details: None,
                        ..Default::default()
                    },
                )
                .await
            {
                if let Some(rewards) = block.rewards {
                    let slot_rewards: u64 =
                        rewards.iter().map(|reward| reward.lamports as u64).sum();
                    total_rewards += slot_rewards;
                }
            }
            pb.inc(1);

            // Sleep after every 10 slots to avoid rate limiting
            slot_counter += 1;
            if slot_counter % 10 == 0 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }

        println!("Total reward lamports: {}", total_rewards);

        // Calculate stake pool's share (total_rewards_pct is in basis points - 1/100th of a percent)
        let stake_pool_rewards = (total_rewards as u128 * total_rewards_pct as u128) / 10_000;
        println!(
            "Stake pool rewards: {} lamports ({} bps of total rewards)",
            stake_pool_rewards, total_rewards_pct
        );

        // Calculate LST holders' share
        let lst_rewards = (stake_pool_rewards * lst_rewards_pct as u128) / 10_000;
        println!(
            "LST holder rewards: {} lamports ({} bps of stake pool rewards)",
            lst_rewards, lst_rewards_pct
        );

        let stake_pool_pubkey = PubkeySrc::parse(&stake_pool_pubkey).unwrap().pubkey();
        let stake_pool_account: Account = rpc.get_account(&stake_pool_pubkey).await.unwrap();

        let stake_pool_program_id = stake_pool_account.owner;

        let stake_pool: StakePool =
            StakePool::deserialize(&mut stake_pool_account.data.as_slice()).unwrap();

        let StakePool {
            validator_list,
            reserve_stake,
            pool_mint,
            manager_fee_account,
            token_program,
            ..
        } = deserialize_stake_pool_checked(stake_pool_account.data().as_ref()).unwrap();

        let (withdraw_authority, _bump) = FindWithdrawAuthority {
            pool: stake_pool_pubkey,
        }
        .run_for_prog(&stake_pool_program_id);

        let final_ixs = vec![
            // Transfer rewards to Stake Pool reserve
            transfer(
                &identity_pubkey,
                &stake_pool.reserve_stake,
                lst_rewards.try_into().unwrap(),
            ),
            // Update stake pool balance
            update_stake_pool_balance_ix_with_program_id(
                stake_pool_program_id,
                UpdateStakePoolBalanceKeys {
                    stake_pool: stake_pool_pubkey,
                    withdraw_authority,
                    validator_list,
                    reserve_stake,
                    manager_fee_account,
                    pool_mint,
                    token_program,
                },
            )
            .unwrap(),
        ];

        let final_ixs = match send_mode {
            TxSendMode::DumpMsg => final_ixs,
            _ => {
                with_auto_cb_ixs(
                    &rpc,
                    &identity_keypair.pubkey(),
                    final_ixs,
                    &[],
                    fee_limit_cb,
                )
                .await
            }
        };

        println!("Final transaction: ");
        handle_tx_full(&rpc, send_mode, &final_ixs, &[], &mut [&identity_keypair]).await;
    }
}
