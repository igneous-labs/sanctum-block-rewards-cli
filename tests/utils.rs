use sanctum_block_rewards_cli::checked_pct;
use sanctum_block_rewards_cli::get_total_block_rewards_for_slots;
use sanctum_block_rewards_cli::SOLANA_PUBLIC_RPC;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

#[test]
fn test_checked_pct() {
    // Test basic percentage calculations
    assert_eq!(checked_pct(100, 5000), Some(50)); // 50% of 100
    assert_eq!(checked_pct(1000, 2500), Some(250)); // 25% of 1000
    assert_eq!(checked_pct(500, 1000), Some(50)); // 10% of 500

    // Test 0% and 100%
    assert_eq!(checked_pct(100, 0), Some(0)); // 0% of anything is 0
    assert_eq!(checked_pct(100, 10000), Some(100)); // 100% of value

    // Test with 0 value
    assert_eq!(checked_pct(0, 5000), Some(0)); // 50% of 0 is 0

    // Test overflow cases
    assert_eq!(checked_pct(u64::MAX, 10000), None); // Should overflow
    assert_eq!(checked_pct(u64::MAX, 5000), None); // Should overflow
}

#[tokio::test]
async fn test_get_total_block_rewards_for_slots_skipped_slot() {
    let rpc = RpcClient::new_with_commitment(
        SOLANA_PUBLIC_RPC.to_string(),
        CommitmentConfig::confirmed(),
    );

    let slots = vec![322368304];
    let total_rewards = get_total_block_rewards_for_slots(&rpc, &slots)
        .await
        .unwrap();

    // Since the slot was skipped, total rewards should be 0
    assert_eq!(total_rewards, 0);
}

#[tokio::test]
async fn test_get_total_block_rewards_for_slots_valid_block() {
    let rpc = RpcClient::new_with_commitment(
        SOLANA_PUBLIC_RPC.to_string(),
        CommitmentConfig::confirmed(),
    );

    let slots = vec![322272000];
    let total_rewards = get_total_block_rewards_for_slots(&rpc, &slots)
        .await
        .unwrap();

    // This block exists and should have non-zero rewards
    assert!(
        total_rewards > 0,
        "Expected non-zero rewards for valid block"
    );
}
