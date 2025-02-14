# ☁️ Sanctum Block Rewards CLI 

A command-line tool for Solana validators to manage and distribute block rewards between stake pools and LST holders.

## Installation

In case you don't have Rust installed, please follow the instructions [here](https://www.rust-lang.org/tools/install) to install Rust. Once you have Rust installed, you can proceed to install the CLI.

### Clone the repository

```bash
git clone https://github.com/igneous-labs/sanctum-block-rewards-cli.git
cd sanctum-block-rewards-cli
```

### Install the CLI

```bash
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

Usage: sanctum-rewards calculate [OPTIONS] --identity-keypair-path <IDENTITY_KEYPAIR_PATH>

Options:
      --identity-keypair-path <IDENTITY_KEYPAIR_PATH>
          The identity keypair of your validator

      --epoch <EPOCH>
          The epoch to calculate rewards for

  -h, --help
          Print help (see a summary with '-h')
```

This command:
- Fetches block rewards data for the specified epoch
- Calculates total rewards earned
- Saves the results to a local JSON file for later use


### `transfer`

```bash
Transfer block rewards to the stake pool reserve

Usage: sanctum-rewards transfer [OPTIONS] --identity-keypair-path <IDENTITY_KEYPAIR_PATH>

Options:
      --identity-keypair-path <IDENTITY_KEYPAIR_PATH>
          The identity keypair for your validator

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

### `sign`

```bash
Sign message to endorse your Sanctum LST

Usage: sanctum-rewards sign --identity-keypair-path <IDENTITY_KEYPAIR_PATH>

Options:
      --identity-keypair-path <IDENTITY_KEYPAIR_PATH>
          The identity keypair for your validator

  -h, --help
          Print help (see a summary with '-h')
```

This command:
- Prompts the user to sign the message
- Prints the signed message
- Reach out to us on Telegram @fp_lee with your signed message
