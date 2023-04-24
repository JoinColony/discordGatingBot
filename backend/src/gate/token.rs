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
    /// The token address on the Gnosis chain
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
        "Guards a role with a token balance on the Gnosis chain"
    }

    fn options() -> Vec<GateOption> {
        vec![
            GateOption {
                name: "token_address",
                description: "The token address on the Gnosis chain",
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

        if amount <= 0 {
            return Err(
                anyhow!("Amount must be greater than 0").context("Failed to create token gate")
            );
        }
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
        if let Err(_) = CLIENT.set(client) {
            warn!("Reputation gate client already set");
        }
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
    use table_test::table_test;

    #[derive(Debug)]
    struct MockColonyTokenClient {}
    impl MockColonyTokenClient {
        fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl ColonyTokenClient for MockColonyTokenClient {
        async fn balance_of(&self, token_address: &H160, wallet_address: &H160) -> Result<U256> {
            if token_address
                != &H160::from_str("0x0000000000000000000000000000000000000001").unwrap()
                && token_address
                    != &H160::from_str("0x000000000000000000000000000000000000000A").unwrap()
            {
                return Ok(U256::from(0));
            }
            if wallet_address
                == &H160::from_str("0x0000000000000000000000000000000000000001").unwrap()
            {
                return Ok(U256::from(1));
            }
            if wallet_address
                == &H160::from_str("0x000000000000000000000000000000000000000A").unwrap()
            {
                return Ok(U256::from(10));
            }
            bail!("Invalid token address")
        }

        async fn get_token_decimals(&self, token_address: &H160) -> Result<u8> {
            if token_address
                == &H160::from_str("0x0000000000000000000000000000000000000001").unwrap()
            {
                return Ok(0);
            }
            if token_address
                == &H160::from_str("0x000000000000000000000000000000000000000A").unwrap()
            {
                return Ok(0);
            }
            bail!("Invalid token address")
        }

        async fn get_token_symbol(&self, token_address: &H160) -> Result<String> {
            if token_address
                == &H160::from_str("0x0000000000000000000000000000000000000001").unwrap()
            {
                return Ok("".to_string());
            }
            if token_address
                == &H160::from_str("0x000000000000000000000000000000000000000A").unwrap()
            {
                return Ok("TEST".to_string());
            }
            bail!("Invalid token address")
        }
    }

    fn setup() {
        let client = Arc::new(MockColonyTokenClient::new());
        TokenGate::init_client(client);
    }

    #[test]
    fn test_name() {
        assert_eq!(TokenGate::name(), "token");
    }

    #[tokio::test]
    async fn test_instance_name() {
        setup();
        let mut options = Vec::with_capacity(2);
        options.push(GateOptionValue {
            name: "token_address".to_string(),
            value: GateOptionValueType::String(
                "0x0000000000000000000000000000000000000001".to_string(),
            ),
        });
        options.push(GateOptionValue {
            name: "amount".to_string(),
            value: GateOptionValueType::I64(1),
        });
        let gate = Gate::new(1, "token", &options).await.unwrap();
        assert_eq!(TokenGate::name(), gate.name());
    }

    #[test]
    fn test_description() {
        assert_eq!(
            TokenGate::description(),
            "Guards a role with a token balance on the Gnosis chain"
        );
    }

    #[test]
    fn test_options() {
        let options = TokenGate::options();
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].name, "token_address");
        assert_eq!(
            options[0].description,
            "The token address on the Gnosis chain"
        );
        assert_eq!(options[0].required, true);
        assert_eq!(options[1].name, "amount");
        assert_eq!(options[1].description, "The amount of the token");
        assert_eq!(options[1].required, true);
    }

    #[tokio::test]
    async fn test_from_wrong_number_of_options() {
        setup();
        let mut options = Vec::with_capacity(3);

        options.push(GateOptionValue {
            name: "token_address".to_string(),
            value: GateOptionValueType::String(
                "0x0000000000000000000000000000000000000001".to_string(),
            ),
        });
        assert!(Gate::new(1, "token", &options).await.is_err());
        options.push(GateOptionValue {
            name: "amount".to_string(),
            value: GateOptionValueType::I64(1),
        });
        assert!(Gate::new(1, "token", &options).await.is_ok());
        options.push(GateOptionValue {
            name: "amount".to_string(),
            value: GateOptionValueType::I64(1),
        });
        assert!(Gate::new(1, "token", &options).await.is_err());
    }

    #[tokio::test]
    async fn test_from_unordered_options() {
        setup();
        let mut options = Vec::with_capacity(2);
        options.push(GateOptionValue {
            name: "amount".to_string(),
            value: GateOptionValueType::I64(1),
        });
        options.push(GateOptionValue {
            name: "token_address".to_string(),
            value: GateOptionValueType::String(
                "0x0000000000000000000000000000000000000001".to_string(),
            ),
        });
        assert!(Gate::new(1, "token", &options).await.is_err());
    }

    #[tokio::test]
    async fn test_token_gate_from_options() {
        setup();
        let cases = vec![
            (
                ("0x0000000000000000000000000000000000000001", 1),
                Ok(("0x64", "0x0000000000000000000000000000000000000001", "", 1)),
            ),
            (
                ("0x000000000000000000000000000000000000000A", 1),
                Ok((
                    "0x64",
                    "0x000000000000000000000000000000000000000a",
                    "TEST",
                    1,
                )),
            ),
            (("0x000000000000000000000000000000000000DEAD", 1), Err(())),
            (("0xc9B6218AffE8Aba68a13899Cbf7cF7f14DDd304C", 1), Err(())),
            (("0x0000000000000000000000000000000000000001", 0), Err(())),
            (("0x0000000000000000000000000000000000000001", -1), Err(())),
        ];

        for (test_case, (address, amount), expected) in table_test!(cases) {
            let mut options = Vec::with_capacity(2);
            options.push(GateOptionValue {
                name: "token_address".to_string(),
                value: GateOptionValueType::String(address.to_string()),
            });
            options.push(GateOptionValue {
                name: "amount".to_string(),
                value: GateOptionValueType::I64(amount),
            });
            match Gate::new(1, "token", &options).await {
                Ok(gate) => {
                    let fields = gate.condition.fields();
                    let actual_chain_id =
                        if let GateOptionValueType::String(value) = &fields[0].value {
                            value
                        } else {
                            panic!("Invalid option type");
                        };
                    let actual_address =
                        if let GateOptionValueType::String(value) = &fields[1].value {
                            value
                        } else {
                            panic!("Invalid option type");
                        };
                    let actual_symbol = if let GateOptionValueType::String(value) = &fields[2].value
                    {
                        value
                    } else {
                        panic!("Invalid option type");
                    };
                    let actual_amount = if let GateOptionValueType::I64(value) = &fields[3].value {
                        value
                    } else {
                        panic!("Invalid option type");
                    };
                    if let Ok((exp_chain_id, exp_token_address, exp_token_symbol, exp_amount)) =
                        expected
                    {
                        test_case
                            .given(&format!(
                                "valid options address: {:?}, amount: {}",
                                address, amount
                            ))
                            .when("creating a gate from options")
                            .then("it should succeed and have the expected fields")
                            .assert_eq(actual_chain_id, &exp_chain_id.to_string())
                            .assert_eq(actual_address, &exp_token_address.to_string())
                            .assert_eq(actual_symbol, &exp_token_symbol.to_string())
                            .assert_eq(actual_amount, &exp_amount);
                    } else {
                        test_case
                            .given(&format!(
                                "valid options address: {:?}, amount: {}",
                                address, amount
                            ))
                            .when("creating a gate from options")
                            .then("it should succeed")
                            .assert_eq(expected.is_ok(), true);
                    }
                }
                Err(_) => {
                    test_case
                        .given(&format!(
                            "invalid options address: {:?}, amount: {}",
                            address, amount
                        ))
                        .when("creating a gate from options")
                        .then("it should give an error")
                        .assert_eq(expected.is_err(), true);
                }
            }
        }
    }
    #[tokio::test]
    async fn test_token_gate_check() {
        setup();
        let cases = vec![
            (
                (
                    "0x000000000000000000000000000000000000000A",
                    9,
                    "0x000000000000000000000000000000000000000A",
                ),
                Some(1234),
            ),
            (
                (
                    "0x000000000000000000000000000000000000000A",
                    11,
                    "0x000000000000000000000000000000000000000A",
                ),
                None,
            ),
            (
                (
                    "0x0000000000000000000000000000000000000001",
                    1,
                    "0x000000000000000000000000000000000000000A",
                ),
                Some(1234),
            ),
            (
                (
                    "0x0000000000000000000000000000000000000001",
                    2,
                    "0x0000000000000000000000000000000000000001",
                ),
                None,
            ),
            (
                (
                    "0x0000000000000000000000000000000000000001",
                    1,
                    "0x000000000000000000000000000000000000DEAD",
                ),
                None,
            ),
        ];
        for (test_case, (address, amount, wallet), expected) in table_test!(cases) {
            let mut options = Vec::with_capacity(2);
            options.push(GateOptionValue {
                name: "token_address".to_string(),
                value: GateOptionValueType::String(address.to_string()),
            });
            options.push(GateOptionValue {
                name: "amount".to_string(),
                value: GateOptionValueType::I64(amount),
            });

            if let Ok(gate) = Gate::new(1234, "token", &options).await {
                let wallet_parsed = H160::from_str(wallet).unwrap();
                let check_result = gate.check_condition(wallet_parsed).await;

                test_case
                    .given(&format!(
                        "valid options address: {:?}, amount: {}, wallet {:?}",
                        address, amount, wallet
                    ))
                    .when("checking the gate condition")
                    .then("it should succeed and allow the right roles")
                    .assert_eq(check_result, expected);
            } else {
                test_case
                    .given(&format!(
                        "invalid options address: {:?}, amount: {}, wallet {:?}",
                        address, amount, wallet
                    ))
                    .when("checking the gate condition")
                    .then("it should fail")
                    .assert_eq(expected, None);
            }
        }
    }
}
