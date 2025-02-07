use sanctum_solana_cli_utils::{
    HandleTxArgs, RecentBlockhash, TxSendMode, TxSendingNonblockingRpcClient,
};
use sanctum_solana_client_utils::{
    buffer_compute_units, calc_compute_unit_price, estimate_compute_unit_limit_nonblocking,
    to_est_cu_sim_tx, SortedSigners,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::{v0::Message, VersionedMessage},
    pubkey::Pubkey,
    signer::Signer,
    transaction::VersionedTransaction,
};

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
    rpc.handle_tx(
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
