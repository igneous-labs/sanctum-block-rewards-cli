use borsh::BorshDeserialize;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use sanctum_solana_cli_utils::{
    HandleTxArgs, RecentBlockhash, TxSendMode, TxSendingNonblockingRpcClient,
};
use sanctum_solana_client_utils::{
    buffer_compute_units, calc_compute_unit_price, estimate_compute_unit_limit_nonblocking,
    to_est_cu_sim_tx, SortedSigners,
};
use sanctum_spl_stake_pool_lib::{deserialize_stake_pool_checked, FindWithdrawAuthority};
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcBlockConfig, RpcLeaderScheduleConfig},
};
use solana_sdk::{
    account::ReadableAccount,
    address_lookup_table::AddressLookupTableAccount,
    compute_budget::ComputeBudgetInstruction,
    epoch_schedule::{EpochSchedule, MINIMUM_SLOTS_PER_EPOCH},
    instruction::Instruction,
    message::{v0::Message, VersionedMessage},
    pubkey::Pubkey,
    signer::Signer,
    system_instruction::transfer,
    transaction::VersionedTransaction,
};
use spl_stake_pool_interface::{
    update_stake_pool_balance_ix_with_program_id, StakePool, UpdateStakePoolBalanceKeys,
};
use std::fmt::Write;

const CU_BUFFER_RATIO: f64 = 1.1;
const CUS_REQUIRED_FOR_SET_CU_LIMIT_IXS: u32 = 300;

pub async fn with_auto_cb_ixs(
    rpc: &RpcClient,
    payer_pk: &Pubkey,
    mut ixs: Vec<Instruction>,
    luts: &[AddressLookupTableAccount],
    fee_limit_cb_lamports: u64,
) -> Vec<Instruction> {
    if fee_limit_cb_lamports == 0 {
        return ixs;
    }
    let tx_to_sim = to_est_cu_sim_tx(payer_pk, &ixs, luts).unwrap();
    let units_consumed = estimate_compute_unit_limit_nonblocking(rpc, &tx_to_sim)
        .await
        .unwrap();
    let units_consumed = buffer_compute_units(units_consumed, CU_BUFFER_RATIO)
        .saturating_add(CUS_REQUIRED_FOR_SET_CU_LIMIT_IXS);
    let microlamports_per_cu = calc_compute_unit_price(units_consumed, fee_limit_cb_lamports);
    ixs.insert(
        0,
        ComputeBudgetInstruction::set_compute_unit_limit(units_consumed),
    );
    ixs.insert(
        0,
        ComputeBudgetInstruction::set_compute_unit_price(microlamports_per_cu),
    );
    ixs
}

/// First signer in signers is transaction payer
pub async fn handle_tx_full(
    rpc: &RpcClient,
    send_mode: TxSendMode,
    ixs: &[Instruction],
    luts: &[AddressLookupTableAccount],
    signers: &mut [&dyn Signer],
) {
    let payer_pk = signers[0].pubkey();
    signers.sort_by_key(|s| s.pubkey());
    let RecentBlockhash { hash, .. } = rpc.get_confirmed_blockhash().await.unwrap();
    return rpc
        .handle_tx(
            &VersionedTransaction::try_new(
                VersionedMessage::V0(Message::try_compile(&payer_pk, ixs, luts, hash).unwrap()),
                &SortedSigners(signers),
            )
            .unwrap(),
            send_mode,
            HandleTxArgs::cli_default(),
        )
        .await
        .unwrap();
}

pub fn get_first_slot_of_epoch(epoch: u64, epoch_schedule: &EpochSchedule) -> u64 {
    if epoch <= epoch_schedule.first_normal_epoch {
        (1u64 << epoch) * MINIMUM_SLOTS_PER_EPOCH
    } else {
        (epoch - epoch_schedule.first_normal_epoch) * epoch_schedule.slots_per_epoch
            + epoch_schedule.first_normal_slot
    }
}

pub async fn get_leader_slots_for_identity(
    rpc: &RpcClient,
    epoch: u64,
    epoch_schedule: &EpochSchedule,
    // TODO(sk): Remove hard coded identity
    _identity_pubkey: &Pubkey,
) -> Vec<u64> {
    let epoch_first_slot = get_first_slot_of_epoch(epoch, epoch_schedule);

    let epoch_leader_schedule = rpc
        .get_leader_schedule_with_config(
            Some(epoch_first_slot),
            RpcLeaderScheduleConfig {
                identity: Some("SDEVqCDyc3YzjrDn375SMWKpZo1m7tbZ12fsenF48x1".to_string()), // TODO(sk): Remove hard coded identity
                commitment: Some(rpc.commitment()),
            },
        )
        .await
        .unwrap();

    if epoch_leader_schedule.is_none() {
        return [].to_vec();
    }

    let relative_leader_slots = epoch_leader_schedule
        .unwrap()
        .get("SDEVqCDyc3YzjrDn375SMWKpZo1m7tbZ12fsenF48x1")
        .unwrap_or(&vec![])
        .to_vec();

    // Map relative leader slots to absolute slots
    let mut leader_slots = Vec::with_capacity(relative_leader_slots.len());
    for &relative_leader_slot in relative_leader_slots.iter() {
        let absolute_slot = epoch_first_slot + relative_leader_slot as u64;
        leader_slots.push(absolute_slot);
    }

    return leader_slots;
}

pub async fn get_total_block_rewards_for_slots(rpc: &RpcClient, slots: &[u64]) -> u64 {
    let mut total_rewards = 0u64;

    let pb = ProgressBar::new(slots.len() as u64);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} slots ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));

    for &slot in slots.iter() {
        let block = rpc
            .get_block_with_config(
                slot as u64,
                RpcBlockConfig {
                    rewards: Some(true),
                    commitment: Some(rpc.commitment()),
                    max_supported_transaction_version: Some(0),
                    transaction_details: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        if let Some(rewards) = block.rewards {
            let slot_rewards: u64 = rewards.iter().map(|reward| reward.lamports as u64).sum();
            total_rewards += slot_rewards;
        }

        pb.inc(1);
    }

    total_rewards
}

pub async fn transfer_to_reserve_and_update_stake_pool_balance_ixs(
    rpc: &RpcClient,
    identity_pubkey: &Pubkey,
    stake_pool_pubkey: &Pubkey,
    lst_rewards: u64,
) -> Vec<Instruction> {
    let stake_pool_account = rpc.get_account(&stake_pool_pubkey).await.unwrap();

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
        pool: *stake_pool_pubkey,
    }
    .run_for_prog(&stake_pool_program_id);

    let final_ixs = vec![
        // Transfer rewards to Stake Pool reserve
        transfer(
            identity_pubkey,
            &stake_pool.reserve_stake,
            lst_rewards.try_into().unwrap(),
        ),
        // Update stake pool balance
        update_stake_pool_balance_ix_with_program_id(
            stake_pool_program_id,
            UpdateStakePoolBalanceKeys {
                stake_pool: *stake_pool_pubkey,
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

    final_ixs
}
