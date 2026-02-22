# ☁️ Sanctum Validator APY CLI

A command-line tool for Solana validators to calculate real APY (inflation + block rewards) and manage block reward distribution between stake pools and LST holders.

## Installation

In case you don't have Rust installed, please follow the instructions [here](https://www.rust-lang.org/tools/install) to install Rust. Once you have Rust installed, you can proceed to install the CLI.

### Clone the repository

```bash
git clone https://github.com/igneous-labs/sanctum-validator-apy-cli.git
```

### Install the CLI

```bash
# Navigate to the cloned repository
cd sanctum-validator-apy-cli

# Run cargo install
cargo install --path . --locked 
```

### Verify the installation

```bash
sanctum-validator-apy --help
```

## Commands

### `calculate`

```bash
Calculate the total block rewards earned by your validator for a specific epoch.

Usage: sanctum-validator-apy calculate [OPTIONS]

Options:
      --identity-pubkey <IDENTITY_PUBKEY>
          The identity pubkey of your validator

      --epoch <EPOCH>
          The epoch to calculate rewards for

  -h, --help
          Print help (see a summary with '-h')
```

This command:
- Fetches block rewards data for the specified epoch
- Calculates total rewards earned
- Saves the results to a local JSON file for later use

### `calculate-with-dune`

```bash
Calculate the total block rewards earned by your validator for a specific epoch.

Usage: sanctum-validator-apy calculate-with-dune [OPTIONS]

Options:
      --identity-pubkey <IDENTITY_PUBKEY>
          The identity pubkey of your validator

      --dune-api-key <DUNE_API_KEY>
          Dune API key

      --epoch <EPOCH>
          The epoch to calculate rewards for

      --timeout <TIMEOUT>
          Timeout in seconds for waiting for query results (default: 300)
          
          [default: 300]

  -h, --help
          Print help (see a summary with '-h')
```

This command:
- Fetches block rewards data for the specified epoch using our public [Dune query](https://dune.com/queries/4745888)
- Saves the results to a local JSON file for later use

> [!NOTE]
> The data on Dune is usually lagging by 2-3 hours, so please make sure you consider this when using this command.

### `calculate-apy`

```bash
Calculate the real APY (inflation + block rewards) for your validator over a range of epochs.

Usage: sanctum-validator-apy calculate-apy [OPTIONS]

Options:
      --identity-pubkey <IDENTITY_PUBKEY>
          The identity pubkey of your validator

      --vote-account <VOTE_ACCOUNT>
          The vote account pubkey of your validator

      --dune-api-key <DUNE_API_KEY>
          Dune API key

      --helius-api-key <HELIUS_API_KEY>
          Helius API key (for fetching inflation rewards)

      --start-epoch <START_EPOCH>
          The start epoch to calculate apy for

      --end-epoch <END_EPOCH>
          The end epoch to calculate apy for

      --timeout <TIMEOUT>
          Timeout in seconds for waiting for query results (default: 300)

          [default: 300]

  -h, --help
          Print help (see a summary with '-h')
```

This command:
- Fetches per-epoch block rewards from [Dune Analytics](https://dune.com/queries/6723900) (or loads from cache)
- Fetches all stake account inflation rewards per epoch via Helius RPC (`getInflationReward`)
- Retrieves per-epoch active stake from the [Stakewiz API](https://stakewiz.com)
- Computes: `epoch_yield = (total_inflation + block_reward) / total_active_stake`
- Annualizes with compounding: `APY = (1 + avg_epoch_yield) ^ 182.5 - 1`

> [!NOTE]
> Requires both a [Dune API key](https://dune.com/settings/api) and a [Helius API key](https://www.helius.dev/).

### `transfer`

```bash
Transfer block rewards to the stake pool reserve

Usage: sanctum-validator-apy transfer [OPTIONS] --payer <PAYER>

Options:
      --payer <PAYER>
          Path to the keypair from where rewards will be transferred

      --identity-pubkey <IDENTITY_PUBKEY>
          The identity pubkey of your validator

      --epoch <EPOCH>
          The epoch to calculate rewards for

      --stake-pool-pubkey <STAKE_POOL_PUBKEY>
          The stake pool account linked to your LST

      --total-rewards-pct <TOTAL_REWARDS_PCT>
          Percentage of stake you want to consider for calculating the block rewards

      --lst-rewards-pct <LST_REWARDS_PCT>
          Percentage of block rewards to share to LST holders

  -h, --help
          Print help (see a summary with '-h')
```

This command:
- Loads previously calculated rewards data
- Transfers the specified percentage of rewards to the stake pool reserve
- Updates stake pool balance by calling `UpdateStakePoolBalance` instruction