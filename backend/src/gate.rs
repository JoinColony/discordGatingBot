use anyhow::{bail, Result};
use async_trait::async_trait;
use colony_rs::H160;
use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};
use std::boxed::Box;
use std::fmt::Display;
mod reputation;
pub use reputation::ReputationGate;
mod token;
pub use token::TokenGate;

/// This macro gives us a way to access associated functions on all GatingConditions
/// A new GatingCondition must be added to This macro to be useful in different
/// parts of the application.
#[macro_export]
macro_rules! gates {
    (@names: $($gate:ident),*) => {
        vec![$($crate::gate::$gate::name()),*]
    };

    (@descriptions: $($gate:ident),*) => {
        {
            use $crate::gate::GatingCondition;
            let mut description_map = std::collections::HashMap::new();
            $(description_map.insert($crate::gate::$gate::name(), $crate::gate::$gate::description());)*
            description_map
        }
    };

    (@options: $($gate:ident),*) => {
        {
            use $crate::gate::GatingCondition;
            let mut option_map = std::collections::HashMap::new();
            $(option_map.insert($crate::gate::$gate::name(), $crate::gate::$gate::options());)*
            option_map
        }
    };

    (@construct $type:ident $options:ident : $($gate:ident),*) => {
        (|| -> Result<Box<dyn $crate::gate::GatingCondition>> {
            $(
                if $crate::gate::$gate::name() == $type {
                return Ok($crate::gate::$gate::from_options(&$options)? as Box<dyn $crate::gate::GatingCondition>);
            })*
            bail!("Unknown gate type: {}", $type)
        })()
    };

    ($($slector:ident)*) => {
        // Here new gating conditions can be added as long as they implement the
        // GatingCondition trait.
        gates!(@$($slector)*: ReputationGate, TokenGate)
    };
}

#[derive(Clone, Debug, Eq, Deserialize, Serialize)]
pub struct Gate {
    /// The role to be granted
    pub role_id: u64,
    pub condition: Box<dyn GatingCondition>,
}

impl Gate {
    pub fn new(role_id: u64, gate_type: &str, options: &Vec<GateOptionValue>) -> Result<Self> {
        let condition = gates!(construct gate_type options)?;
        Ok(Self { role_id, condition })
    }

    pub async fn check(self, address: H160) -> Option<u64> {
        if self.condition.check(address).await {
            Some(self.role_id)
        } else {
            None
        }
    }

    pub fn identifier(&self) -> u128 {
        let h = self.condition.hashed();
        (self.role_id as u128) << 64 | h as u128
    }
}

impl PartialEq for Gate {
    fn eq(&self, other: &Self) -> bool {
        self.identifier() == other.identifier()
    }
}

#[typetag::serde]
#[async_trait]
pub trait GatingCondition: std::fmt::Debug + Send + Sync + DynClone {
    fn name() -> &'static str
    where
        Self: Sized;
    fn description() -> &'static str
    where
        Self: Sized;
    fn options() -> Vec<GateOption>
    where
        Self: Sized;
    fn from_options(options: &Vec<GateOptionValue>) -> Result<Box<Self>>
    where
        Self: Sized;
    async fn check(&self, wallet_address: H160) -> bool;
    fn hashed(&self) -> u64;
    fn fields(&self) -> Vec<GateOptionValue>;
}

dyn_clone::clone_trait_object!(GatingCondition);

impl Eq for Box<dyn GatingCondition> {}

impl PartialEq for Box<dyn GatingCondition> {
    fn eq(&self, other: &Box<dyn GatingCondition>) -> bool {
        self.hashed() == other.hashed()
    }
}

#[derive(Debug, Clone)]
pub struct GateOption {
    pub name: &'static str,
    pub description: &'static str,
    pub option_type: GateOptionType,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub enum GateOptionType {
    I64 {
        min: Option<u64>,
        max: Option<u64>,
    },
    F64 {
        min: Option<f64>,
        max: Option<f64>,
    },
    String {
        min_length: Option<u16>,
        max_length: Option<u16>,
    },
}

#[derive(Debug, Clone)]
pub struct GateOptionValue {
    pub name: String,
    pub value: GateOptionValueType,
}

#[derive(Debug, Clone)]
pub enum GateOptionValueType {
    I64(i64),
    F64(f64),
    String(String),
}

impl Display for GateOptionValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateOptionValueType::I64(i) => write!(f, "{}", i),
            GateOptionValueType::F64(n) => write!(f, "{}", n),
            GateOptionValueType::String(s) => write!(f, "{}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reputation_gate_from_options() {
        let mut options = Vec::with_capacity(3);

        options.push(GateOptionValue {
            name: "colony".to_string(),
            value: GateOptionValueType::String(
                "0xCFD3aa1EbC6119D80Ed47955a87A9d9C281A97B3".to_string(),
            ),
        });
        options.push(GateOptionValue {
            name: "domain".to_string(),
            value: GateOptionValueType::I64(1),
        });
        options.push(GateOptionValue {
            name: "reputation".to_string(),
            value: GateOptionValueType::I64(0),
        });
        let gate = Gate::new(1, "reputation", &options).unwrap();
        assert_eq!(gate.role_id, 1);
        let fields = gate.condition.fields();
        let colony = if let GateOptionValueType::String(value) = &fields[0].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(colony, "0xCFD3aa1EbC6119D80Ed47955a87A9d9C281A97B3");
        let domain = if let GateOptionValueType::I64(value) = &fields[1].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(*domain, 1);
        let reputation = if let GateOptionValueType::I64(value) = &fields[2].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(*reputation, 0);
    }

    #[test]
    fn test_token_gate_from_options() {
        let mut options = Vec::with_capacity(2);
        options.push(GateOptionValue {
            name: "token_address".to_string(),
            value: GateOptionValueType::String(
                "0xCFD3aa1EbC6119D80Ed47955a87A9d9C281A97B3".to_string(),
            ),
        });
        options.push(GateOptionValue {
            name: "amount".to_string(),
            value: GateOptionValueType::I64(1),
        });
        let gate = Gate::new(1, "token", &options).unwrap();
        assert_eq!(gate.role_id, 1);
        let fields = gate.condition.fields();
        let address = if let GateOptionValueType::String(value) = &fields[0].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(address, "0xCFD3aa1EbC6119D80Ed47955a87A9d9C281A97B3");
        let amount = if let GateOptionValueType::I64(value) = &fields[1].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(*amount, 1);
    }

    #[test]
    fn test_gate_macros() {
        let names = gates!(names);
        assert_eq!(names, vec!["reputation", "token"]);
        let option_map = gates!(options);
        eprintln!("{:#?}", option_map);
        assert_eq!(option_map.len(), 2);
        assert_eq!(option_map["reputation"].len(), 3);
        assert_eq!(option_map["token"].len(), 2);
    }
}
