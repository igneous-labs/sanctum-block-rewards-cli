use sanctum_block_rewards_cli::checked_pct;

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
