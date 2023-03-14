use crate::config::CONFIG;
use crate::gate::{
    GateOption, GateOptionType, GateOptionValue, GateOptionValueType, GatingCondition,
};
use anyhow::{bail, Result};
use async_trait::async_trait;
use cached::{proc_macro::cached, Cached, TimedCache};
use colony_rs::{get_reputation_in_domain, H160, U512};
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use nonzero_ext::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::boxed::Box;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};

pub static RATE_LIMITER: Lazy<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> =
    Lazy::new(|| RateLimiter::direct(Quota::per_second(nonzero!(100u32))));
/// Represents a gate for a discord role issues by the /gate slash command.
/// This is stored in the database for each discord server.
#[derive(Debug, Clone, Deserialize, Hash, Serialize, PartialEq, Eq)]
pub struct ReputationGate {
    /// The colony address in which the reputation should be looked up
    pub colony: H160,
    /// The domain in which the reputation should be looked up  
    pub domain: u64,
    /// The reputation amount required to be granted the role
    pub reputation: u32,
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
        let mut options = Vec::with_capacity(3);
        options.push(GateOption {
            name: &"colony",
            description: &"The colony address in which the reputation should be looked up",
            required: true,
            option_type: GateOptionType::String {
                min_length: Some(42),
                max_length: Some(42),
            },
        });
        options.push(GateOption {
            name: &"domain",
            description: &"The domain in which the reputation should be looked up",
            required: true,
            option_type: GateOptionType::I64 {
                min: Some(1),
                max: None,
            },
        });
        options.push(GateOption {
            name: &"reputation",
            description: &"The reputation amount required to be granted the role",
            required: true,
            option_type: GateOptionType::F64 {
                min: Some(0.0),
                max: Some(100.0),
            },
        });
        options
    }
    fn from_options(options: &Vec<GateOptionValue>) -> Result<Box<Self>> {
        if options.len() != 3 {
            bail!("Need exactly 3 options");
        }
        if options[0].name != "colony" {
            bail!("First option must be colony");
        }
        let colony = match &options[0].value {
            GateOptionValueType::String(s) => H160::from_str(&s)?,
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
        let precision = CONFIG.wait().precision;
        // TODO: can we make precision a u32 in the first place?
        let factor = 10u32.pow(precision as u32);
        let reputation = match &options[2].value {
            GateOptionValueType::F64(i) => *i as u32,
            _ => bail!("Invalid option type, expected float for reputation"),
        };

        if reputation > 100 * factor {
            bail!("Reputation must be 100 or less")
        }

        Ok(Box::new(ReputationGate {
            colony,
            domain: domain as u64,
            reputation,
        }))
    }

    async fn check(&self, wallet_address: H160) -> bool {
        let reputation_percentage =
            match check_reputation(wallet_address, self.colony, self.domain).await {
                Ok(percentage) => percentage,
                Err(why) => {
                    info!("Error checking reputation: {:?}", why);
                    return false;
                }
            };
        debug!(
            "Reputation percentage: {} for wallet: {:?}",
            reputation_percentage, wallet_address
        );
        let guard = COLONY_CACHE.lock().await;
        let hits = guard.cache_hits();
        let misses = guard.cache_misses();
        let size = guard.cache_size();
        debug!(
            "Colony reputation cache hits: {:?}, misses: {:?}, size: {}",
            hits, misses, size
        );
        if reputation_percentage >= self.reputation {
            true
        } else {
            false
        }
    }

    fn hashed(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn fields(&self) -> Vec<GateOptionValue> {
        let precision = CONFIG.wait().precision;
        let factor = 10.0f64.powi(-(precision as i32));
        let reputation = self.reputation as f64 * factor;
        vec![
            GateOptionValue {
                name: "colony".to_string(),
                value: GateOptionValueType::String(format!("{:?}", self.colony)),
            },
            GateOptionValue {
                name: "domain".to_string(),
                value: GateOptionValueType::I64(self.domain as i64),
            },
            GateOptionValue {
                name: "reputation".to_string(),
                value: GateOptionValueType::F64(reputation),
            },
        ]
    }
}

/// This is used to gather the fraction of total reputation a wallet has in
/// a domain in a colony
async fn check_reputation(wallet: H160, colony: H160, domain: u64) -> Result<u32> {
    debug!(
        "Checking reputation for wallet {:?} in colony {:?} domain {}",
        wallet, colony, domain
    );
    let mut interval = tokio::time::interval(Duration::from_millis(1));
    loop {
        interval.tick().await;
        {
            let mut guard = COLONY_CACHE.lock().await;
            // we only check the user for a cache hit, this should imply a
            // cache hit for the base reputation as well, edge cases should
            // be irrelevant
            if let Some(_result) = guard.cache_get(&(colony, wallet, domain)) {
                debug!(
                    "Cache hit for colony {} wallet {} domain {}, can return now",
                    colony, wallet, domain
                );
                break;
            }
        }
        // we need a double ticket here, because we need to check the base
        // reputation and the user reputation separately
        match RATE_LIMITER.check_n(nonzero!(2u32)) {
            Ok(_) => {
                debug!(
                    "Got pass from rate limiter for colony {} wallet {} domain {}, can return now",
                    colony, wallet, domain
                );
                break;
            }
            Err(_) => trace!("Rate limit reached, waiting"),
        }
    }

    let base_reputation_fut = tokio::spawn(async move {
        let colony_address = colony.clone();
        let zero_address = colony_rs::Address::zero();
        get_reputation_in_domain_cached(&colony_address, &zero_address, domain).await
    });
    let user_reputation_fut =
        tokio::spawn(
            async move { get_reputation_in_domain_cached(&colony, &wallet, domain).await },
        );

    let (base_result, user_result) = tokio::join!(base_reputation_fut, user_reputation_fut);
    let base_reputation_str = match base_result.expect("Panicked in base reputation") {
        Ok(reputation) => reputation,
        Err(why) => {
            warn!("Failed to get base reputation: {:?}", why);
            bail!("Failed to get base reputation");
        }
    };

    debug!("Base reputation: {}", base_reputation_str);
    let user_reputation_str = match user_result.expect("Panicked in user reputation") {
        Ok(reputation) => reputation,
        Err(why) => {
            info!("Failed to get user reputation: {:?}", why);
            "0".to_string()
        }
    };
    Ok(calculate_reputation_percentage(
        &base_reputation_str,
        &user_reputation_str,
    ))
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
    match get_reputation_in_domain(colony_address, wallet_address, domain).await {
        Ok(rep_no_proof) => Ok(rep_no_proof.reputation_amount),
        Err(why) => Err(format!("{:?}", why)),
    }
}

fn calculate_reputation_percentage(base_reputation_str: &str, user_reputation_str: &str) -> u32 {
    // Since we have big ints for the reputation and a reputation threshold
    // in percent, we need to do some math to get the correct result
    // also the precision of the reputation threshold is variable
    let base_reputation = U512::from_dec_str(&base_reputation_str).unwrap();
    let user_reputation = U512::from_dec_str(&user_reputation_str).unwrap();
    let precision = CONFIG.wait().precision;
    let factor = U512::from(10).pow(U512::from(precision)) * U512::from(100);
    let reputation = user_reputation * factor / base_reputation;
    reputation.as_u32()
}
