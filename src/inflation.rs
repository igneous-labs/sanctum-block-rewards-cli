use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use stakewiz_rs::{QueryParams, StakewizClient, EpochStake};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

type Epoch = u64;
type BlockReward = u64;
type APY = f64;
pub type EpochBlockReward = (Epoch, BlockReward); // must be sorted by epoch - low epoch first
type EpochAPY = (Epoch, APY);

const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;
const INFLATION_REWARD_BATCH_SIZE: usize = 100;

pub async fn get_real_apy(
    vote_account: &str,
    epochblockreward_vec: Vec<EpochBlockReward>,
    current_epoch: u64,
    helius_rpc_url: &str,
) -> Result<f64, Box<dyn std::error::Error>> {
    let mut epochapy_vec: Vec<EpochAPY> = Vec::new();

    let client = StakewizClient::new();
    let epoch_count = (current_epoch.saturating_sub(epochblockreward_vec[0].0) + 1) as u32;
    let query_params = QueryParams::new()
        .sort_desc("epoch")
        .limit(epoch_count);
    let epochstake_vec: Vec<EpochStake> = client
        .get_validator_total_stakes(vote_account, Some(&query_params))
        .await?;

    let reward_epochs: HashSet<u64> = epochblockreward_vec.iter().map(|(e, _)| *e).collect();

    let filtered_epochstake_vec: Vec<&EpochStake> = epochstake_vec
        .iter()
        .filter(|es| reward_epochs.contains(&es.epoch))
        .collect();

    if filtered_epochstake_vec.len() != epochblockreward_vec.len() {
        return Err(format!(
            "filtered epoch stake length ({}) != epoch block rewards length ({})",
            filtered_epochstake_vec.len(),
            epochblockreward_vec.len()
        )
        .into());
    }

    // Build a map for epoch -> block_reward (lamports)
    let reward_map: HashMap<u64, u64> = epochblockreward_vec.into_iter().collect();

    // Get all stake account pubkeys for the validator
    let stakeaccounts_vec = client.get_validator_stakes(vote_account).await?;
    let stake_pubkeys: Vec<Pubkey> = stakeaccounts_vec
        .iter()
        .filter_map(|sa| Pubkey::from_str(&sa.pubkey).ok())
        .collect();

    eprintln!("[INFO] Found {} stake accounts to query", stake_pubkeys.len());

    let rpc = RpcClient::new(helius_rpc_url.to_string());

    for epochstake in &filtered_epochstake_vec {
        let epoch = epochstake.epoch;

        // Batch getInflationReward for all stake accounts
        let mut total_inflation_lamports: u64 = 0;

        for chunk in stake_pubkeys.chunks(INFLATION_REWARD_BATCH_SIZE) {
            let rewards = rpc.get_inflation_reward(chunk, Some(epoch)).await?;

            for reward in rewards.iter().flatten() {
                total_inflation_lamports += reward.amount;
            }
        }

        let total_inflation_sol = total_inflation_lamports as f64 / LAMPORTS_PER_SOL;
        let block_reward_sol = reward_map[&epoch] as f64 / LAMPORTS_PER_SOL;
        let total_active_stake_sol = epochstake.stake;

        let epoch_yield = (total_inflation_sol + block_reward_sol) / total_active_stake_sol;

        eprintln!(
            "[DEBUG] epoch={} active_stake={:.2} inflation={:.4} SOL block_reward={:.4} SOL yield={:.6}",
            epoch, total_active_stake_sol, total_inflation_sol, block_reward_sol, epoch_yield
        );

        epochapy_vec.push((epoch, epoch_yield));
    }

    // Average per-epoch yield, then annualize with compounding
    let avg_epoch_yield: f64 = epochapy_vec.iter().map(|(_, r)| r).sum::<f64>()
        / epochapy_vec.len() as f64;

    // ~182.5 epochs per year (365 days / 2 days per epoch)
    // Solana epoch = 432,000 slots × 400ms = 172,800s = 2 days
    let epochs_per_year: f64 = 182.5;
    let annual_apy = (1.0 + avg_epoch_yield).powf(epochs_per_year) - 1.0;

    Ok(annual_apy)
}
