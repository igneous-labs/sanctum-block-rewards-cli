# ☁️ Sanctum Block Rewards CLI 

A command-line tool for Solana validators to manage and distribute block rewards between stake pools and LST holders.

## Installation

In case you don't have Rust installed, please follow the instructions [here](https://www.rust-lang.org/tools/install) to install Rust. Once you have Rust installed, you can proceed to install the CLI.

### Clone the repository

```bash
git clone https://github.com/igneous-labs/sanctum-block-rewards-cli.git
```

### Install the CLI

```bash
# Navigate to the cloned repository
cd sanctum-block-rewards-cli

# Run cargo install
cargo install --path . --locked 
```

### Verify the installation

```bash
sanctum-rewards --help
```

## Commands

### `calculate`

```bash
Calculate the total block rewards earned by your validator for a specific epoch.

Usage: sanctum-rewards calculate [OPTIONS]

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

Usage: sanctum-rewards calculate-with-dune [OPTIONS]

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


### `transfer`

```bash
Transfer block rewards to the stake pool reserve

Usage: sanctum-rewards transfer [OPTIONS] --payer <PAYER>

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