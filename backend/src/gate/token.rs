use crate::gate::{
    GateOption, GateOptionType, GateOptionValue, GateOptionValueType, GatingCondition,
};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use colony_rs::{H160, U256};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::boxed::Box;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, instrument, warn, Instrument};

static CLIENT: OnceCell<Arc<dyn ColonyTokenClient>> = OnceCell::new();

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
        vec![
            GateOption {
                name: "token_address",
                description: "The token address on the gnosis chain",
                required: true,
                option_type: GateOptionType::String {
                    min_length: Some(42),
                    max_length: Some(42),
                },
            },
            GateOption {
                name: "amount",
                description: "The amount of the token",
                required: true,
                option_type: GateOptionType::I64 {
                    min: Some(1),
                    max: None,
                },
            },
        ]
    }

    #[instrument(level = "debug")]
    async fn from_options(options: &[GateOptionValue]) -> Result<Box<Self>> {
        debug!("Creating token gate from options");
        if options.len() != 2 {
            bail!("Need exactly 2 options");
        }
        if options[0].name != "token_address" {
            bail!("First option must be token_address");
        }
        let token_address = match &options[0].value {
            GateOptionValueType::String(s) => {
                H160::from_str(s).context("Failed to create token gate, invalid address")?
            }
            _ => bail!("Invalid option type"),
        };
        if options[1].name != "amount" {
            bail!("Second option must be amount");
        }
        let amount = match &options[1].value {
            GateOptionValueType::I64(i) => *i,
            _ => return Err(anyhow!("Invalid option type").context("Failed to create token gate")),
        };
        let chain_id = U256::from(100);

        let token_symbol = CLIENT
            .get()
            .ok_or_else(|| anyhow!("No client set for token gate"))?
            .get_token_symbol(&token_address)
            .await
            .unwrap_or_else(|why| {
                warn!("Failed to get token symbol: {}", why);
                "".to_string()
            });
        debug!(token_symbol, "Token symbol is:");
        let token_decimals = CLIENT
            .get()
            .ok_or_else(|| anyhow!("No client set for token gate"))?
            .get_token_decimals(&token_address)
            .await
            .context("Failed to create token gate, could not get token decimals")?;

        debug!(token_decimals, "Got token decimals:");

        debug!("Done creating token gate from options");
        Ok(Box::new(TokenGate {
            chain_id,
            token_address,
            token_symbol,
            token_decimals,
            amount: amount as u64,
        }))
    }

    #[instrument(name = "token_condition", skip(wallet_address))]
    async fn check(&self, wallet_address: H160) -> bool {
        let Some(client) = CLIENT.get() else {
            error!("No client set for token gate");
            return false;
        };
        let balance = match client
            .balance_of(&self.token_address, &wallet_address)
            .in_current_span()
            .await
        {
            Ok(b) => b,
            Err(why) => {
                warn!("Failed to get balance: {}", why);
                return false;
            }
        };
        debug!(?balance, "Got token");
        let amount_scaled =
            U256::from(self.amount) * U256::from(10).pow(self.token_decimals.into());
        debug!(?amount_scaled, "Scaled amount");
        amount_scaled <= balance
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
                value: GateOptionValueType::String(self.token_symbol.to_string()),
            },
            GateOptionValue {
                name: "amount".to_string(),
                value: GateOptionValueType::I64(self.amount as i64),
            },
        ]
    }

    fn instance_name(&self) -> &'static str {
        Self::name()
    }
}

impl TokenGate {
    pub fn init_client<C: 'static + ColonyTokenClient>(client: Arc<C>) {
        CLIENT.set(client).expect("Failed to set client");
    }
}

#[async_trait]
pub trait ColonyTokenClient: std::fmt::Debug + Send + Sync {
    async fn balance_of(&self, token_address: &H160, wallet_address: &H160) -> Result<U256>;
    async fn get_token_decimals(&self, wallet_address: &H160) -> Result<u8>;
    async fn get_token_symbol(&self, wallet_address: &H160) -> Result<String>;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::gate::Gate;
    use async_trait::async_trait;

    #[derive(Debug)]
    struct MockColonyTokenClient {
        reputation: String,
    }
    impl MockColonyTokenClient {
        fn new(reputation: String) -> Self {
            Self { reputation }
        }
    }

    #[async_trait]
    impl ColonyTokenClient for MockColonyTokenClient {
        async fn balance_of(&self, _token_address: &H160, _wallet_address: &H160) -> Result<U256> {
            Ok(U256::from(100))
        }

        async fn get_token_decimals(&self, _wallet_address: &H160) -> Result<u8> {
            Ok(18)
        }

        async fn get_token_symbol(&self, _wallet_address: &H160) -> Result<String> {
            Ok("TEST".to_string())
        }
    }

    fn setup() {
        let client = Arc::new(MockColonyTokenClient::new("100".to_string()));
        TokenGate::init_client(client);
    }

    #[tokio::test]
    async fn test_token_gate_from_options() {
        setup();
        let mut options = Vec::with_capacity(2);
        options.push(GateOptionValue {
            name: "token_address".to_string(),
            value: GateOptionValueType::String(
                "0xc9B6218AffE8Aba68a13899Cbf7cF7f14DDd304C".to_string(),
            ),
        });
        options.push(GateOptionValue {
            name: "amount".to_string(),
            value: GateOptionValueType::I64(1),
        });
        let gate = Gate::new(1, "token", &options).await.unwrap();
        assert_eq!(gate.role_id, 1);
        let fields = gate.condition.fields();
        let chain_id = if let GateOptionValueType::String(value) = &fields[0].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(chain_id, "0x64");
        let address = if let GateOptionValueType::String(value) = &fields[1].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(address, "0xc9b6218affe8aba68a13899cbf7cf7f14ddd304c");
        let symbol = if let GateOptionValueType::String(value) = &fields[2].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(symbol, "TEST");
        let amount = if let GateOptionValueType::I64(value) = &fields[3].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(*amount, 1);
    }
}
