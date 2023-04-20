use crate::gate::{
    GateOption, GateOptionType, GateOptionValue, GateOptionValueType, GatingCondition,
};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use cached::{proc_macro::cached, Cached, TimedCache};
use colony_rs::{u256_from_f64_saturating, ReputationNoProof, H160, U256, U512};
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use nonzero_ext::*;
use once_cell::sync::{Lazy, OnceCell};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::Duration;
use std::{boxed::Box, sync::Arc};
use tracing::{debug, info, instrument, trace, warn, Instrument};

/// this must be smaller than 1e76 or so, to not overflow the later U512
/// multiplications
pub const PRECISION_FACTOR: f64 = (std::u128::MAX >> 1) as f64 / 100.0;
/// this must be smaller than 1e78 or so, to not overflow the U512
/// multiplications
static PRECISION_FACTOR_TIMES_100: Lazy<U512> = Lazy::new(|| U512::from(std::u128::MAX >> 1));

pub static RATE_LIMITER: Lazy<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> =
    Lazy::new(|| RateLimiter::direct(Quota::per_second(nonzero!(100u32))));

static CLIENT: OnceCell<Arc<dyn ColonyReputationClient>> = OnceCell::new();

/// Represents a gate for a discord role issues by the /gate slash command.
/// This is stored in the database for each discord server.
#[derive(Debug, Clone, Deserialize, Hash, Serialize, PartialEq, Eq)]
pub struct ReputationGate {
    chain_id: U256,
    /// The colony address in which the reputation should be looked up
    colony_address: H160,
    colony_name: String,
    /// The domain in which the reputation should be looked up
    colony_domain: u64,
    /// The reputation percentage in a domain required to be granted the role
    /// scaled by the precision factor to not lose everything after the comma in
    /// the f64 conversion
    reputation_threshold_scaled: U256,
}

#[typetag::serde]
#[async_trait]
impl GatingCondition for ReputationGate {
    fn name() -> &'static str {
        "reputation"
    }
    fn description() -> &'static str {
        "Guards a role with a reputation percentage in a colony domain"
    }
    fn options() -> Vec<GateOption> {
        vec![
            GateOption {
                name: "colony",
                description: "The colony address in which the reputation should be looked up",
                required: true,
                option_type: GateOptionType::String {
                    min_length: Some(42),
                    max_length: Some(42),
                },
            },
            GateOption {
                name: "domain",
                description: "The domain in which the reputation should be looked up",
                required: true,
                option_type: GateOptionType::I64 {
                    min: Some(1),
                    max: None,
                },
            },
            GateOption {
                name: "reputation",
                description: "The reputation amount required to be granted the role",
                required: true,
                option_type: GateOptionType::F64 {
                    min: Some(0.0),
                    max: Some(100.0),
                },
            },
        ]
    }
    #[instrument(level = "info")]
    async fn from_options(options: &[GateOptionValue]) -> Result<Box<Self>> {
        debug!("Creating reputation gate from options");
        if options.len() != 3 {
            bail!("Need exactly 3 options");
        }
        if options[0].name != "colony" {
            bail!("First option must be colony");
        }
        let colony_address = match &options[0].value {
            GateOptionValueType::String(s) => {
                H160::from_str(s).context("Failed to create reputation gate, invalid address")?
            }
            _ => bail!("Invalid option type, expected string for colony address"),
        };
        if options[1].name != "domain" {
            bail!("Second option must be domain");
        }
        let domain = match &options[1].value {
            GateOptionValueType::I64(i) => *i,
            _ => bail!("Invalid option type, expected integer for domain"),
        };
        if domain < 1 {
            bail!("Domain must be greater than 0");
        }
        if options[2].name != "reputation" {
            bail!("Third option must be reputation");
        }

        let domaincount = CLIENT
            .get()
            .ok_or_else(|| anyhow!("No client set for reputation gate"))?
            .get_domain_count(&colony_address)
            .in_current_span()
            .await
            .context("Failed to create reputation gate, could not get domains for colony")?;

        if domain as u64 > domaincount {
            bail!("The domain number is higher than the domain count in the colony");
        }

        let reputation_percentage = match &options[2].value {
            GateOptionValueType::F64(f) => *f,
            _ => bail!("Invalid option type, expected float for reputation"),
        };
        if reputation_percentage > 100.0 {
            bail!("Reputation must be 100 or less")
        }
        if reputation_percentage <= 0.0 {
            bail!("Reputation must be more than 0")
        }
        let reputation_threshold_scaled =
            u256_from_f64_saturating(reputation_percentage * PRECISION_FACTOR);

        let colony_name = CLIENT
            .get()
            .ok_or_else(|| anyhow!("No client set for reputation gate"))?
            .get_colony_name(&colony_address)
            .await
            .unwrap_or_else(|why| {
                warn!("Error getting colony name: {}", why);
                "".to_string()
            });
        debug!(?colony_name, "Colony name is:");

        let chain_id = U256::from(100);
        debug!("Done creating reputation gate from options");

        Ok(Box::new(ReputationGate {
            chain_id,
            colony_address,
            colony_name,
            colony_domain: domain as u64,
            reputation_threshold_scaled,
        }))
    }

    #[instrument(name = "reputation_condition", skip(wallet_address))]
    async fn check(&self, wallet_address: H160) -> bool {
        debug!("Checking reputation gate");
        check_reputation(
            self.reputation_threshold_scaled,
            wallet_address,
            self.colony_address,
            self.colony_domain,
        )
        .in_current_span()
        .await
        .unwrap_or_else(|why| {
            warn!("Error checking reputation: {}", why);
            false
        })
    }

    fn hashed(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn fields(&self) -> Vec<GateOptionValue> {
        // This should not panic, since we validate the options to be lower than
        // 100 and the precision factor must be < u128::MAX / 100 for this to
        // work reliably with conversion errors
        let reputation = self.reputation_threshold_scaled.as_u128() as f64 / PRECISION_FACTOR;
        vec![
            GateOptionValue {
                name: "chain_id".to_string(),
                value: GateOptionValueType::String(format!("{:#x}", self.chain_id)),
            },
            GateOptionValue {
                name: "colony_address".to_string(),
                value: GateOptionValueType::String(format!("{:?}", self.colony_address)),
            },
            GateOptionValue {
                name: "colony_name".to_string(),
                value: GateOptionValueType::String(self.colony_name.to_string()),
            },
            GateOptionValue {
                name: "domain".to_string(),
                value: GateOptionValueType::I64(self.colony_domain as i64),
            },
            GateOptionValue {
                name: "reputation".to_string(),
                value: GateOptionValueType::F64(reputation),
            },
        ]
    }

    fn instance_name(&self) -> &'static str {
        Self::name()
    }
}

/// This is used to gather the fraction of total reputation a wallet has in
/// a domain in a colony
#[instrument(level = "debug", skip(wallet))]
async fn check_reputation(
    reputation_percentage: U256,
    wallet: H160,
    colony: H160,
    domain: u64,
) -> Result<bool> {
    debug!("Checking reputation");
    let mut interval = tokio::time::interval(Duration::from_millis(1));
    loop {
        trace!("Waiting for rate limiter");
        interval.tick().in_current_span().await;
        {
            trace!("Waiting for cache lock");
            let mut guard = COLONY_CACHE.lock().in_current_span().await;
            // we only check the user for a cache hit, this should imply a
            // cache hit for the base reputation as well, edge cases should
            // be irrelevant
            if guard.cache_get(&(colony, wallet, domain)).is_some() {
                debug!("Cache hit, can return now");
                break;
            }
        }
        // we need a double ticket here, because we need to check the base
        // reputation and the user reputation separately
        match RATE_LIMITER.check_n(nonzero!(2u32)) {
            Ok(_) => {
                break;
            }
            Err(_) => trace!("Rate limit reached, waiting"),
        }
    }
    debug!("Passed rate limiting");
    let base_reputation_fut = tokio::spawn(async move {
        let colony_address = colony;
        let zero_address = colony_rs::Address::zero();
        get_reputation_in_domain_cached(&colony_address, &zero_address, domain)
            .in_current_span()
            .await
    });
    let user_reputation_fut = tokio::spawn(async move {
        get_reputation_in_domain_cached(&colony, &wallet, domain)
            .in_current_span()
            .await
    });
    let (base_result, user_result) = tokio::join!(base_reputation_fut, user_reputation_fut);
    let base_reputation_str = match base_result? {
        Ok(reputation) => reputation,
        Err(why) => {
            warn!("Failed to get base reputation: {:?}", why);
            bail!("Failed to get base reputation: {:?}", why);
        }
    };

    debug!(reputation = base_reputation_str, "Got base reputation");
    let user_reputation_str = match user_result? {
        Ok(reputation) => reputation,
        Err(why) => {
            info!("Failed to get user reputation: {:?}", why);
            "0".to_string()
        }
    };
    calculate_reputation_percentage(
        reputation_percentage,
        &base_reputation_str,
        &user_reputation_str,
    )
}

#[cached(
    name = "COLONY_CACHE",
    type = "TimedCache<(H160,H160,u64), Result<String, String>>",
    create = r##"{
        TimedCache::with_lifespan_and_refresh(3600, true)
        }
    "##
)]
async fn get_reputation_in_domain_cached(
    colony_address: &H160,
    wallet_address: &H160,
    domain: u64,
) -> Result<String, String> {
    let Some(client) = CLIENT.get() else {
        return Err("No client available".to_string());
    };
    match client
        .get_reputation_in_domain(colony_address, wallet_address, domain)
        .in_current_span()
        .await
    {
        Ok(rep_no_proof) => Ok(rep_no_proof.reputation_amount),
        Err(why) => Err(format!("{:?}", why)),
    }
}

#[instrument(level = "debug")]
fn calculate_reputation_percentage(
    reputation_threshold_scaled: U256,
    base_reputation_str: &str,
    user_reputation_str: &str,
) -> Result<bool> {
    // Since we have big integers for the reputation and a reputation threshold
    // in percent, we can't just build the quotient and compare it to the
    // threshold. Instead we do a little algebra and only multiply integers
    //
    // threshold% * PRECISION_FACTOR <= 100% * PRECISION_FACTOR * user_reputation / base_reputation
    // => threshold * PRECISION_FACTOR * base_reputation <= 100 * PRECISION_FACTOR * user_reputation
    //
    debug!("Calculating reputation percentage",);
    let base_reputation = U512::from_dec_str(base_reputation_str)?;
    let user_reputation = U512::from_dec_str(user_reputation_str)?;
    let reputation_threshold_scaled = U512::from(reputation_threshold_scaled);
    debug!(
        ?base_reputation,
        ?user_reputation,
        ?reputation_threshold_scaled,
        "Converted reputation values"
    );
    let left_side = reputation_threshold_scaled
        .checked_mul(base_reputation)
        .ok_or(anyhow!(
            "Failed to calculate reputation percentage left side, overflow"
        ))?;
    let right_side = PRECISION_FACTOR_TIMES_100
        .checked_mul(user_reputation)
        .ok_or(anyhow!(
            "Failed to calculate reputation percentage right side, overflow"
        ))?;
    debug!(?left_side, ?right_side, "Calculated reputation percentage");
    Ok(left_side <= right_side)
}

impl ReputationGate {
    pub fn init_client<C: 'static + ColonyReputationClient>(client: Arc<C>) {
        CLIENT.set(client).expect("Failed to set client");
    }
}

#[async_trait]
pub trait ColonyReputationClient: std::fmt::Debug + Send + Sync {
    async fn get_reputation_in_domain(
        &self,
        colony_address: &H160,
        wallet_address: &H160,
        domain: u64,
    ) -> Result<ReputationNoProof>;
    async fn get_colony_name(&self, colony_address: &H160) -> Result<String>;
    async fn get_domain_count(&self, colony_address: &H160) -> Result<u64>;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::gate::Gate;
    use async_trait::async_trait;
    use colony_rs::ReputationNoProof;

    #[derive(Debug)]
    struct MockColonyReputationClient {
        reputation: String,
    }
    impl MockColonyReputationClient {
        fn new(reputation: String) -> Self {
            Self { reputation }
        }
    }

    #[async_trait]
    impl ColonyReputationClient for MockColonyReputationClient {
        async fn get_reputation_in_domain(
            &self,
            _colony_address: &H160,
            _wallet_address: &H160,
            _domain: u64,
        ) -> Result<ReputationNoProof> {
            Ok(ReputationNoProof {
                key: "0x000000".to_string(),
                reputation_amount: "10".to_string(),
                value: "0".to_string(),
            })
        }

        async fn get_colony_name(&self, colony_address: &H160) -> Result<String> {
            Ok("TestColony".to_string())
        }

        async fn get_domain_count(&self, colony_address: &H160) -> Result<u64> {
            Ok(3)
        }
    }

    fn setup() {
        let client = Arc::new(MockColonyReputationClient::new("100".to_string()));
        ReputationGate::init_client(client);
    }

    #[tokio::test]
    async fn test_reputation_gate_from_options() {
        setup();
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
            value: GateOptionValueType::F64(0.1),
        });
        let gate = Gate::new(1, "reputation", &options).await.unwrap();
        assert_eq!(gate.role_id, 1);
        let fields = gate.condition.fields();
        let chain_id = if let GateOptionValueType::String(value) = &fields[0].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(chain_id, "0x64");
        let colony = if let GateOptionValueType::String(value) = &fields[1].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(colony, "0xcfd3aa1ebc6119d80ed47955a87a9d9c281a97b3");
        let colony_name = if let GateOptionValueType::String(value) = &fields[2].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(colony_name, "TestColony");
        let domain = if let GateOptionValueType::I64(value) = &fields[3].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(*domain, 1);
        let reputation = if let GateOptionValueType::F64(value) = &fields[4].value {
            value
        } else {
            panic!("Invalid option type");
        };
        assert_eq!(*reputation, 0.1);
    }
}
