use crate::gate::{
    GateOption, GateOptionType, GateOptionValue, GateOptionValueType, GatingCondition,
};
use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use cached::{proc_macro::cached, Cached, TimedCache};
use colony_rs::{get_reputation_in_domain, u256_from_f64_saturating, H160, U256, U512};
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use nonzero_ext::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::Duration;
use std::{boxed::Box, ops::Sub};
use tracing::{debug, error, info, trace, warn};

/// this must be smaller than 1e76 or so, to not overflow the U512
/// multiplications
pub const PRECISION_FACTOR: f64 = (std::u128::MAX / 2) as f64 / 100.0;
/// this must be smaller than 1e78 or so, to not overflow the U512
/// multiplications
static PRECISION_FACTOR_TIMES_100: Lazy<U512> = Lazy::new(|| U512::from(std::u128::MAX / 2));

pub static RATE_LIMITER: Lazy<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> =
    Lazy::new(|| RateLimiter::direct(Quota::per_second(nonzero!(100u32))));
/// Represents a gate for a discord role issues by the /gate slash command.
/// This is stored in the database for each discord server.
#[derive(Debug, Clone, Deserialize, Hash, Serialize, PartialEq, Eq)]
pub struct ReputationGate {
    /// The colony address in which the reputation should be looked up
    pub colony_address: H160,
    /// The domain in which the reputation should be looked up  
    pub colony_domain: u64,
    /// The reputation percentage in a domain required to be granted the role
    /// scaled by the precision factor to not lose everything after the comma in
    /// the f64 conversion
    pub reputation_threshold_scaled: U256,
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
        let colony_address = match &options[0].value {
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
        let reputation_percentage = match &options[2].value {
            GateOptionValueType::F64(f) => *f,
            _ => bail!("Invalid option type, expected float for reputation"),
        };
        if reputation_percentage > 100.0 {
            bail!("Reputation must be 100 or less")
        }
        if reputation_percentage < 0.0 {
            bail!("Reputation must be 0 or more")
        }
        let reputation_threshold_scaled =
            u256_from_f64_saturating(reputation_percentage * PRECISION_FACTOR);

        Ok(Box::new(ReputationGate {
            colony_address,
            colony_domain: domain as u64,
            reputation_threshold_scaled,
        }))
    }

    async fn check(&self, wallet_address: H160) -> bool {
        check_reputation(
            self.reputation_threshold_scaled,
            wallet_address,
            self.colony_address,
            self.colony_domain,
        )
        .await
        .unwrap_or_else(|why| {
            error!("Error checking reputation: {}", why);
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
                name: "colony".to_string(),
                value: GateOptionValueType::String(format!("{:?}", self.colony_address)),
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
}

/// This is used to gather the fraction of total reputation a wallet has in
/// a domain in a colony
async fn check_reputation(
    reputation_percentage: U256,
    wallet: H160,
    colony: H160,
    domain: u64,
) -> Result<bool> {
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
    match get_reputation_in_domain(colony_address, wallet_address, domain).await {
        Ok(rep_no_proof) => Ok(rep_no_proof.reputation_amount),
        Err(why) => Err(format!("{:?}", why)),
    }
}

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
    let base_reputation = U512::from_dec_str(&base_reputation_str)?;
    let user_reputation = U512::from_dec_str(&user_reputation_str)?;
    let reputation_threshold_scaled = U512::from(reputation_threshold_scaled);
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
    Ok(left_side <= right_side)
}
