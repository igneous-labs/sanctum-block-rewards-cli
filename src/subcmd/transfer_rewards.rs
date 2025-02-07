use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use sanctum_spl_stake_pool_lib::{
    account_resolvers::UpdateStakePoolBalance, deserialize_stake_pool_checked,
    FindWithdrawAuthority,
};
use solana_readonly_account::{keyed::Keyed, ReadonlyAccountData};
use spl_stake_pool_interface::{
    update_stake_pool_balance_ix, update_stake_pool_balance_ix_with_program_id, StakePool,
    UpdateStakePoolBalanceKeys, ValidatorList,
};
use std::fmt::Write;
use std::sync::Arc;

use borsh::BorshDeserialize;

use crate::{handle_tx_full, subcmd::Subcmd, with_auto_cb_ixs};
use clap::{command, Args};
use sanctum_solana_cli_utils::{parse_named_signer, ParseNamedSigner, PubkeySrc, TxSendMode};
use solana_client::rpc_config::{RpcBlockConfig, RpcLeaderScheduleConfig};
use solana_sdk::{
    account::{Account, ReadableAccount},
    commitment_config::CommitmentConfig,
    fee,
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
            stake_pool_pubkey,
            // total_rewards_pct,
            // lst_rewards_pct,
        } = match args.subcmd {
            Subcmd::TransferRewards(a) => a,
        };

        let rpc = args.config.nonblocking_rpc_client();
        let send_mode = args.send_mode;
        let fee_limit_cb = args.fee_limit_cb;

        let payer = args.config.signer();

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

        let pb = ProgressBar::new(num_leader_slots);
        pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} slots ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));

        // Arcs to share across threads
        let pb = Arc::new(pb);
        let rpc = Arc::new(rpc);

        let reward_lamports = std::sync::atomic::AtomicU64::new(0);

        // Split the leader slots into chunks of `SLOT_CHUNK_SIZE` slots and process them in parallel
        let chunks = relative_leader_slots.chunks(SLOT_CHUNK_SIZE);

        for chunk in chunks {
            let futures = chunk.iter().map(|&leader_slot| {
                let rpc = rpc.clone();
                let reward_lamports = &reward_lamports;
                let pb = pb.clone();
                async move {
                    let absolute_slot = previous_epoch_first_slot + leader_slot as u64;

                    if let Ok(block) = rpc
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
                    {
                        if let Some(rewards) = block.rewards {
                            let chunk_rewards: u64 =
                                rewards.iter().map(|reward| reward.lamports as u64).sum();
                            reward_lamports
                                .fetch_add(chunk_rewards, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    pb.inc(1);
                }
            });

            futures::future::join_all(futures).await;
        }

        let total_rewards = reward_lamports.load(std::sync::atomic::Ordering::Relaxed);
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

        let validator_list_account = rpc.get_account(&stake_pool.validator_list).await.unwrap();

        let ValidatorList { validators, .. } =
            <ValidatorList as borsh::BorshDeserialize>::deserialize(
                &mut validator_list_account.data.as_slice(),
            )
            .unwrap();

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

        let final_ixs = vec![update_stake_pool_balance_ix_with_program_id(
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
        .unwrap()];

        let final_ixs = match send_mode {
            TxSendMode::DumpMsg => final_ixs,
            _ => with_auto_cb_ixs(&rpc, &payer.pubkey(), final_ixs, &[], fee_limit_cb).await,
        };
        eprintln!("Sending final update tx");
        handle_tx_full(&rpc, send_mode, &final_ixs, &[], &mut [&payer]).await;
    }
}
