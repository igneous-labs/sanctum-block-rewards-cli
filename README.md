# sanctum-block-rewards-cli

A command-line tool for Solana validators to manage and distribute block rewards between stake pools and LST holders.

## Installation

```bash
cargo install sanctum-rewards
```

## Commands

### `calculate`

```bash
Calculate the total block rewards earned by your validator for a specific epoch.

Usage: blockrewards calculate [OPTIONS] --identity-keypair-path <IDENTITY_KEYPAIR_PATH>

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

Usage: blockrewards transfer [OPTIONS] --identity-keypair-path <IDENTITY_KEYPAIR_PATH>

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

Usage: blockrewards sign --identity-keypair-path <IDENTITY_KEYPAIR_PATH>

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
