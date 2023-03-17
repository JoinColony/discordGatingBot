use crate::gate::{
    GateOption, GateOptionType, GateOptionValue, GateOptionValueType, GatingCondition,
};
use anyhow::{bail, Result};
use async_trait::async_trait;
use colony_rs::{balance_off, get_token_decimals, get_token_symbol, H160, U256};
use serde::{Deserialize, Serialize};
use std::boxed::Box;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use tracing::{debug, error, info, trace, warn};

/// Represents a gate for a discord role issues by the /gate slash command.
/// This is stored in the database for each discord server.
#[derive(Debug, Clone, Deserialize, Hash, Serialize, PartialEq, Eq)]
pub struct TokenGate {
    pub chain_id: U256,
    /// The token address on the gnossis chain
    pub token_address: H160,
    pub token_symbol: String,
    pub token_decimals: u8,
    /// The amount of the token held
    pub amount: u64,
}

#[typetag::serde]
#[async_trait]
impl GatingCondition for TokenGate {
    fn name() -> &'static str {
        "token"
    }

    fn description() -> &'static str {
        "Guards a role with a token balance on the gnosis chain"
    }

    fn options() -> Vec<GateOption> {
        let mut options = Vec::with_capacity(2);

        options.push(GateOption {
            name: &"token_address",
            description: &"The token address on the gnosis chain",
            required: true,
            option_type: GateOptionType::String {
                min_length: Some(42),
                max_length: Some(42),
            },
        });
        options.push(GateOption {
            name: &"amount",
            description: &"The amount of the token",
            required: true,
            option_type: GateOptionType::I64 {
                min: Some(0),
                max: None,
            },
        });
        options
    }

    async fn from_options(options: &Vec<GateOptionValue>) -> Result<Box<Self>> {
        if options.len() != 2 {
            bail!("Need exactly 2 options");
        }
        if options[0].name != "token_address" {
            bail!("First option must be token_address");
        }
        let token_address = match &options[0].value {
            GateOptionValueType::String(s) => H160::from_str(s)?,
            _ => bail!("Invalid option type"),
        };
        if options[1].name != "amount" {
            bail!("Second option must be amount");
        }
        let amount = match &options[1].value {
            GateOptionValueType::I64(i) => *i,
            _ => bail!("Invalid option type"),
        };
        let chain_id = U256::from(100);

        let token_symbol = get_token_symbol(token_address).await?;
        let token_decimals = get_token_decimals(token_address).await?;
        Ok(Box::new(TokenGate {
            chain_id,
            token_address,
            token_symbol,
            token_decimals,
            amount: amount as u64,
        }))
    }

    async fn check(&self, wallet_address: H160) -> bool {
        let balance = match balance_off(&self.token_address, &wallet_address).await {
            Ok(b) => b,
            Err(why) => {
                error!("Failed to get balance: {}", why);
                return false;
            }
        };
        debug!(
            "Balance for token {:?} and wallet {:?} is {:?}",
            self.token_address, wallet_address, balance
        );
        U256::from(self.amount) * U256::from(10).pow(self.token_decimals.into()) <= balance
    }

    fn hashed(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn fields(&self) -> Vec<GateOptionValue> {
        vec![
            GateOptionValue {
                name: "chain_id".to_string(),
                value: GateOptionValueType::String(format!("{:#x}", self.chain_id)),
            },
            GateOptionValue {
                name: "token_address".to_string(),
                value: GateOptionValueType::String(format!("{:?}", self.token_address)),
            },
            GateOptionValue {
                name: "token_symbol".to_string(),
                value: GateOptionValueType::String(format!("{:?}", self.token_symbol)),
            },
            GateOptionValue {
                name: "amount".to_string(),
                value: GateOptionValueType::I64(self.amount as i64),
            },
        ]
    }
}
