use colored::Colorize;
use comfy_table::{Attribute, Cell, Color, Table};
use inquire::Text;
use sanctum_solana_cli_utils::TokenAmt;
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const MAX_EPOCH_BACKWARDS_LOOKUP: u64 = 5;

pub fn get_rewards_file_path(identity_pubkey: &Pubkey, epoch: u64) -> Result<String, String> {
    let home_dir = dirs_next::home_dir()
        .ok_or_else(|| "Could not find home directory".to_string())
        .and_then(|dir| {
            dir.to_str()
                .ok_or_else(|| "Invalid home directory path".to_string())
                .map(String::from)
        })?;

    Ok(format!(
        "{}/.local/sanctum/rewards_{}_{}.json",
        home_dir, identity_pubkey, epoch
    ))
}

pub fn checked_pct(value: u64, bps: u64) -> Option<u64> {
    value
        .checked_mul(bps)
        .and_then(|result| result.checked_div(10_000))
}

fn get_input(
    message: &str,
    placeholder: &str,
    initial_value: Option<String>,
    arg_value: Option<String>,
) -> String {
    if let Some(value) = arg_value {
        value
    } else {
        let message_string = message.blue().bold().to_string();

        if let Some(initial) = initial_value {
            Text::new(&message_string)
                .with_placeholder(placeholder)
                .with_initial_value(&initial)
                .prompt()
                .unwrap_or_else(|_| "".to_string())
                .trim()
                .to_string()
        } else {
            Text::new(&message_string)
                .with_placeholder(placeholder)
                .prompt()
                .unwrap_or_else(|_| "".to_string())
                .trim()
                .to_string()
        }
    }
}

pub fn input_string(
    message: &str,
    placeholder: &str,
    initial_value: Option<String>,
    arg_value: Option<String>,
) -> Result<String, String> {
    let input = get_input(message, placeholder, initial_value, arg_value);

    if input.is_empty() {
        return Err(String::from("Error: Please enter a value"));
    }

    Ok(input)
}

pub fn input_with_validation<T>(
    message: &str,
    placeholder: &str,
    initial_value: Option<String>,
    arg_value: Option<String>,
    validator: impl Fn(&str) -> Result<T, String>,
) -> Result<T, String> {
    let input = get_input(message, placeholder, initial_value, arg_value);

    if input.is_empty() {
        return Err(String::from("Error: Please enter a value"));
    }

    validator(&input)
}

pub fn validate_epoch(input: &str, current_epoch: u64) -> Result<u64, String> {
    match input.parse::<u64>() {
        Ok(e) => {
            if e >= current_epoch {
                Err(format!(
                    "Error: Epoch must be one of the last completed epochs (less than {})",
                    current_epoch
                ))
            } else if e < current_epoch.saturating_sub(MAX_EPOCH_BACKWARDS_LOOKUP) {
                Err(format!(
                    "Error: Epoch must be one of the last {} completed epochs (epoch {} to {})",
                    MAX_EPOCH_BACKWARDS_LOOKUP,
                    current_epoch.saturating_sub(MAX_EPOCH_BACKWARDS_LOOKUP),
                    current_epoch - 1
                ))
            } else {
                Ok(e)
            }
        }
        Err(_) => Err("Error: Please enter a valid number".to_string()),
    }
}

pub fn validate_rpc_url(input: &str) -> Result<String, String> {
    if input.starts_with("http://") || input.starts_with("https://") {
        Ok(input.to_string())
    } else {
        Err("Error: Please enter a valid URL starting with http:// or https://".to_string())
    }
}

pub fn validate_bps(input: &str) -> Result<u64, String> {
    // Parse the input as f64 to handle decimals
    match input.parse::<f64>() {
        Ok(percentage) => {
            // Convert percentage to BPS (multiply by 100 to convert to basis points)
            let bps = (percentage * 100.0).round() as u64;

            if bps > 10_000 {
                Err("Error: Percentage cannot exceed 100%".to_string())
            } else {
                Ok(bps)
            }
        }
        Err(_) => Err("Error: Please enter a valid number".to_string()),
    }
}

pub fn validate_pubkey(input: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(input).map_err(|_| "Error: Please enter a valid Solana public key".to_owned())
}

pub struct PrintTransferSummaryArgs {
    pub epoch: u64,
    pub payer_balance: u64,
    pub total_block_rewards: u64,
    pub total_rewards_bps: u64,
    pub stake_pool_rewards: u64,
    pub lst_rewards_bps: u64,
    pub lst_rewards: u64,
}

pub fn print_transfer_summary(args: PrintTransferSummaryArgs) {
    let PrintTransferSummaryArgs {
        epoch,
        payer_balance,
        total_block_rewards,
        total_rewards_bps,
        stake_pool_rewards,
        lst_rewards_bps,
        lst_rewards,
    } = args;

    let mut table = Table::new();
    table
        .set_header(vec![
            Cell::new("Epoch")
                .add_attribute(Attribute::Bold)
                .fg(Color::Blue),
            Cell::new("Total Block Rewards")
                .add_attribute(Attribute::Bold)
                .fg(Color::Blue),
            Cell::new(format!(
                "Stake Pool Rewards ({}%)",
                total_rewards_bps as f64 / 100.0
            ))
            .add_attribute(Attribute::Bold)
            .fg(Color::Blue),
            Cell::new(format!("LST Rewards ({}%)", lst_rewards_bps as f64 / 100.0))
                .add_attribute(Attribute::Bold)
                .fg(Color::Blue),
        ])
        .add_row(vec![
            Cell::new(format!("{}", epoch)),
            Cell::new(format!(
                "{} SOL",
                TokenAmt {
                    amt: total_block_rewards,
                    decimals: 9
                }
            )),
            Cell::new(format!(
                "{} SOL",
                TokenAmt {
                    amt: stake_pool_rewards,
                    decimals: 9
                }
            )),
            Cell::new(format!(
                "{} SOL",
                TokenAmt {
                    amt: lst_rewards,
                    decimals: 9
                }
            )),
        ]);

    println!("{table}");

    println!("{}", "=".repeat(80));

    println!(
        "{}{}",
        "Pre Transfer balance: ".blue().bold(),
        format!(
            "{} SOL",
            TokenAmt {
                amt: payer_balance,
                decimals: 9
            }
        )
        .green()
        .bold()
    );

    println!(
        "{}{}",
        "Post Transfer balance: ".blue().bold(),
        {
            let post_balance = TokenAmt {
                amt: payer_balance.saturating_sub(lst_rewards),
                decimals: 9,
            };
            let formatted = format!("{} SOL", post_balance);
            if post_balance.integer_part() >= 10 {
                formatted.green()
            } else if post_balance.integer_part() >= 3 {
                formatted.yellow()
            } else {
                formatted.red()
            }
        }
        .bold()
    );
}

#[derive(Debug, Deserialize, Default)]
struct Pool {
    #[serde(default)]
    pool: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LstInfo {
    name: String,
    symbol: String,
    #[serde(default)]
    pool: Pool,
}

#[derive(Debug, Deserialize)]
struct LstList {
    sanctum_lst_list: Vec<LstInfo>,
}

pub async fn get_lst_info(pool_pubkey: &str) -> Result<(String, String), String> {
    // Fetch TOML content from GitHub
    let url = "https://raw.githubusercontent.com/igneous-labs/sanctum-lst-list/refs/heads/master/sanctum-lst-list.toml";
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to fetch TOML: {}", e))?;

    let toml_content = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Parse TOML
    let lst_list: LstList =
        toml::from_str(&toml_content).map_err(|e| format!("Failed to parse TOML: {}", e))?;

    // Find matching pool
    for lst in lst_list.sanctum_lst_list {
        if let Some(pool_str) = lst.pool.pool {
            if pool_str == pool_pubkey {
                return Ok((lst.name, lst.symbol));
            }
        }
    }

    Err(format!("No LST found for pool {}", pool_pubkey))
}
