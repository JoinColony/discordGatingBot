//! This is the main busines logic of the application. It registers a global
//! static channel on which other parts of the application can communicate with
//! the controller.
//!

use crate::{config::CONFIG, storage::Storage};
use anyhow::{bail, Result};
use cached::{proc_macro::cached, Cached, TimedCache};
use chacha20poly1305::{
    aead::generic_array::GenericArray,
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use colony_rs::{get_reputation_in_domain, H160, U512};
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use hex;
use nonzero_ext::*;
use once_cell::sync::{Lazy, OnceCell};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use std::{
    hash::Hash,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinSet,
};
use tracing::{debug, error, info, trace, warn};
use urlencoding;

/// The global channel on which the controller can be communicated with
pub static CONTROLLER_CHANNEL: OnceCell<mpsc::Sender<Message>> = OnceCell::new();
pub static RATE_LIMITER: Lazy<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> =
    Lazy::new(|| RateLimiter::direct(Quota::per_second(nonzero!(100u32))));
/// A session encryption key which is used to encrypt the session used for
/// user registration. It is generated once at startup and never changes as
/// long as the application is running.
static SESSION_KEY: OnceCell<Vec<u8>> = OnceCell::new();

/// The message type is the main way for other parts of the application token
/// communicate with the controller. It is an enum with variants for each
/// possible message.
#[derive(Debug)]
pub enum Message {
    List {
        guild_id: u64,
        response: oneshot::Sender<Vec<Gate>>,
    },
    Delete {
        guild_id: u64,
        colony: String,
        domain: u64,
        reputation: u32,
        role_id: u64,
    },
    Gate {
        guild_id: u64,
        colony: String,
        domain: u64,
        reputation: u32,
        role_id: u64,
    },
    Check {
        guild_id: u64,
        user_id: u64,
        username: String,
        response_tx: oneshot::Sender<CheckResponse>,
    },
    Batch {
        guild_id: u64,
        user_ids: Vec<u64>,
        response_tx: mpsc::Sender<CheckResponse>,
    },
    Register {
        user_id: u64,
        wallet: String,
        response_tx: oneshot::Sender<RegisterResponse>,
    },
    Unregister {
        user_id: u64,
        username: String,
        response_tx: oneshot::Sender<UnRegisterResponse>,
    },
    RemovUser {
        user_id: u64,
    },
}

/// The response to a check message, sent back via the oneshot channel in the
/// inbound message.
#[derive(Debug)]
pub enum CheckResponse {
    Grant(Vec<u64>),
    Register(String),
}

/// The response to a register message, sent back via the oneshot channel in the
/// inbound message.
#[derive(Debug)]
pub enum RegisterResponse {
    AlreadyRegistered,
    Success,
}

/// The response to a unregister message, sent back via the oneshot channel in the
/// inbound message.
#[derive(Debug)]
pub enum UnRegisterResponse {
    NotRegistered,
    Unregister(String),
}

/// Represents a gate for a discord role issues by the /gate slash command.
/// This is stored in the database for each discord server.
#[derive(Debug, Clone, Deserialize, Hash, Serialize, PartialEq, Eq)]
pub struct Gate {
    /// The colony address in which the reputation should be looked up
    pub colony: String,
    /// The domain in which the reputation should be looked up  
    pub domain: u64,
    /// The reputation amount required to be granted the role
    pub reputation: u32,
    /// The role to be granted
    pub role_id: u64,
}

/// The main business logic instance. It holds a storage instance and a channel
/// for communication with other parts of the application.
#[derive(Debug)]
pub struct Controller<S: Storage> {
    pub storage: S,
    pub message_tx: mpsc::Sender<Message>,
    message_rx: mpsc::Receiver<Message>,
}

impl<S: Storage + Send + 'static> Controller<S> {
    pub fn new() -> Self {
        let (message_tx, message_rx) = mpsc::channel(1024);

        Controller {
            storage: S::new(),
            message_tx,
            message_rx,
        }
    }

    pub async fn init()
    where
        S: Storage + Send + 'static,
        <S as Storage>::GateIter: Send,
    {
        let key = ChaCha20Poly1305::generate_key(&mut OsRng);
        SESSION_KEY
            .set(key.to_vec())
            .expect("Failed to set session key");
        let controller: Controller<S> = Controller::new();
        CONTROLLER_CHANNEL
            .set(controller.message_tx.clone())
            .expect("Failed to set controller channel");
        controller.spawn().await;
    }

    /// Starts the controller and sets the global static channel for other
    /// parts of the application to communicate with it.
    pub async fn spawn(self)
    where
        S: Storage + Send + 'static,
        <S as Storage>::GateIter: Send,
    {
        let mut controller = self;
        tokio::spawn(async move {
            while let Some(message) = controller.message_rx.recv().await {
                debug!("Received message: {:?}", message);
                match message {
                    Message::Gate {
                        colony,
                        domain,
                        reputation,
                        role_id,
                        guild_id,
                    } => {
                        let gate = Gate {
                            colony,
                            domain,
                            reputation,
                            role_id,
                        };
                        debug!("Adding gate: {:?}", gate);
                        controller.storage.add_gate(&guild_id, gate);
                    }
                    Message::List { guild_id, response } => {
                        debug!("Received list request for guild {}", guild_id);
                        let gates = controller.storage.list_gates(&guild_id).collect();
                        debug!("Sending list response gates {:?}", gates);
                        if let Err(why) = response.send(gates) {
                            error!("Failed to send list response: {:?}", why);
                        }
                    }
                    Message::Delete {
                        guild_id,
                        colony,
                        domain,
                        reputation,
                        role_id,
                    } => {
                        let gate = Gate {
                            colony,
                            domain,
                            reputation,
                            role_id,
                        };
                        debug!("Deleting gate: {:?}", gate);
                        controller.storage.remove_gate(&guild_id, gate);
                    }
                    Message::Check {
                        user_id,
                        username,
                        guild_id,
                        response_tx,
                    } => {
                        debug!("Checking user {} in guild {}", user_id, guild_id);

                        let user_result = controller.storage.get_user(&user_id);
                        let gates = controller.storage.list_gates(&guild_id);
                        let wallet = match user_result {
                            Some(wallet) => wallet,
                            None => {
                                let url = CONFIG.wait().server.url.clone();
                                let session = Session::new(user_id, username);
                                let url = format!(
                                    "{}/register/{}/{}",
                                    url,
                                    urlencoding::encode(&session.username),
                                    session.encode()
                                );
                                if let Err(why) = response_tx.send(CheckResponse::Register(url)) {
                                    error!("Failed to send CheckResponse::Register: {:?}", why);
                                };
                                continue;
                            }
                        };
                        debug!("User {} has wallet {}", user_id, wallet);
                        let granted_roles = check_user(wallet, gates).await;
                        debug!("Granted roles deduped: {:?}", granted_roles);
                        if let Err(why) = response_tx.send(CheckResponse::Grant(granted_roles)) {
                            error!("Failed to send CheckResponse::Grant: {:?}", why);
                        };
                    }
                    Message::Batch {
                        guild_id: _,
                        user_ids: _,
                        response_tx: _,
                    } => todo!(),
                    Message::Register {
                        user_id,
                        wallet,
                        response_tx,
                    } => {
                        debug!("Registering user {} with wallet {}", user_id, wallet);
                        if controller.storage.contains_user(&user_id) {
                            if let Err(why) = response_tx.send(RegisterResponse::AlreadyRegistered)
                            {
                                error!(
                                    "Failed to send RegisterResponse::AlreadyRegistered: {:?}",
                                    why
                                );
                            };
                        } else {
                            controller.storage.add_user(user_id, wallet);
                            if let Err(why) = response_tx.send(RegisterResponse::Success) {
                                error!("Failed to send RegisterResponse::Success: {:?}", why);
                            };
                        }
                    }
                    Message::Unregister {
                        user_id,
                        username,
                        response_tx,
                    } => {
                        debug!("Unregistering user {}", user_id);

                        match controller.storage.get_user(&user_id) {
                            None => {
                                if let Err(why) =
                                    response_tx.send(UnRegisterResponse::NotRegistered)
                                {
                                    error!(
                                        "Failed to send UnregisterResponse::NotRegistered: {:?}",
                                        why
                                    );
                                };
                            }
                            Some(_) => {
                                let url = CONFIG.wait().server.url.clone();
                                let session = Session::new(user_id, username);
                                let url = format!(
                                    "{}/unregister/{}/{}",
                                    url,
                                    urlencoding::encode(&session.username),
                                    session.encode()
                                );
                                if let Err(why) =
                                    response_tx.send(UnRegisterResponse::Unregister(url))
                                {
                                    error!("Failed to send CheckResponse::Register: {:?}", why);
                                };
                                continue;
                            }
                        };
                    }
                    Message::RemovUser { user_id } => {
                        debug!("Removing user {}", user_id);
                        controller.storage.remove_user(&user_id);
                    }
                }
            }
        });
    }
}

pub async fn check_user(wallet: String, gates: impl Iterator<Item = Gate>) -> Vec<u64> {
    let wallet_arc = Arc::new(wallet);
    let mut set = JoinSet::new();
    for gate in gates {
        let wallet = wallet_arc.clone();
        set.spawn(async move {
            let reputation_res = check_reputation(&gate.colony, &wallet, gate.domain).await;
            if let Ok(reputation) = reputation_res {
                if reputation >= gate.reputation {
                    return Some(gate.role_id);
                }
            }
            None
        });
    }
    let mut granted_roles = Vec::new();
    while let Some(reputation_res) = set.join_next().await {
        if let Ok(Some(role_id)) = reputation_res {
            granted_roles.push(role_id);
        }
    }
    debug!("Granted roles: {:?}", granted_roles);
    granted_roles.sort();
    granted_roles.dedup();
    let guard = COLONY_CACHE.lock().await;
    let hits = guard.cache_hits();
    let misses = guard.cache_misses();
    let size = guard.cache_size();
    debug!(
        "Colony reputation cache hits: {:?}, misses: {:?}, size: {}",
        hits, misses, size
    );
    granted_roles
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

/// This is used to gather the fraction of total reputation a wallet has in
/// a domain in a colony
async fn check_reputation(colony: &str, wallet: &str, domain: u64) -> Result<u32, String> {
    debug!(
        "Checking reputation for wallet {} in colony {} domain {}",
        wallet, colony, domain
    );
    let mut interval = tokio::time::interval(Duration::from_millis(1));
    let colony_address = colony_rs::Address::from_str(colony).unwrap();
    let wallet_address = colony_rs::Address::from_str(wallet).unwrap();
    loop {
        interval.tick().await;
        {
            let mut guard = COLONY_CACHE.lock().await;
            // we only check the user for a cache hit, this should imply a
            // cache hit for the base reputation as well, edge cases should
            // be irrelevant
            if let Some(_result) = guard.cache_get(&(colony_address, wallet_address, domain)) {
                debug!(
                    "Cache hit for colony {} wallet {} domain {}, can return now",
                    colony, wallet, domain
                );
                break;
            }
        }

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
        let colony_address = colony_address.clone();
        let zero_address = colony_rs::Address::zero();
        get_reputation_in_domain_cached(&colony_address, &zero_address, domain).await
    });
    let user_reputation_fut = tokio::spawn(async move {
        get_reputation_in_domain_cached(&colony_address, &wallet_address, domain).await
    });

    let (base_result, user_result) = tokio::join!(base_reputation_fut, user_reputation_fut);
    let base_reputation_str = match base_result.expect("Panicked in base reputation") {
        Ok(reputation) => reputation,
        Err(why) => {
            warn!("Failed to get base reputation: {:?}", why);
            return Err("Failed to get base reputation".to_string());
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

/// This represents a session for a user that has not yet registered their
/// and is used to generate a url for the user to register their wallet.
/// The session is encoded as a nonce and string separated by a dot.
/// The string is an encrypted version of the user information
#[derive(Debug, Deserialize, Serialize)]
pub struct Session {
    pub user_id: u64,
    pub username: String,
    pub timestamp: u64,
}

impl Session {
    pub fn new(user_id: u64, username: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get system timestamp")
            .as_secs();
        Session {
            user_id,
            username,
            timestamp,
        }
    }

    pub fn expired(&self) -> bool {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get system timestamp")
            .as_secs();
        timestamp - self.timestamp > 60
    }

    pub fn encode(&self) -> String {
        let plaintext_str = format!("{}:{}:{}", self.user_id, self.username, self.timestamp);

        let plaintext = plaintext_str.as_bytes();
        let key_bytes = SESSION_KEY.wait();
        let key = GenericArray::from_slice(&key_bytes);

        let cipher = ChaCha20Poly1305::new(key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, plaintext).unwrap();
        let encoded_nonce = hex::encode(nonce);
        let encoded_ciphertext = hex::encode(ciphertext);
        format!("{}.{}", encoded_nonce, encoded_ciphertext)
    }
}

impl FromStr for Session {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        // let user_id: u64;
        // let timestamp;

        let key_bytes = SESSION_KEY.wait();
        let key = GenericArray::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);
        let uri_parts: Vec<_> = s.split('.').collect();
        if uri_parts.len() != 2 {
            bail!("Invalid Uri: could not split in two parts");
        }
        let nonce_bytes = hex::decode(uri_parts[0])?;
        let nonce = GenericArray::from_slice(&nonce_bytes);

        let ciphertext = hex::decode(uri_parts[1])?;
        let plaintext = if let Ok(plaintext) = cipher.decrypt(&nonce, ciphertext.as_slice()) {
            plaintext
        } else {
            bail!("Invalid Uri: could not decrypt");
        };
        let plaintext_str = String::from_utf8(plaintext)?;

        let parts: Vec<_> = plaintext_str.split(':').collect();
        if parts.len() != 3 {
            bail!("Invalid session string");
        }
        let user_id = parts[0].parse()?;
        let username = parts[1].parse()?;
        let timestamp = parts[2].parse()?;

        Ok(Self {
            user_id,
            username,
            timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliConfig;
    use crate::config::setup_config;
    use crate::storage;

    async fn setup() {
        let cfg = CliConfig::default();
        setup_config(&cfg).unwrap();
        Controller::<storage::InMemoryStorage>::init().await;
    }

    #[tokio::test]
    async fn test_session() {
        setup().await;
        let session = Session::new(123, "test".to_string());
        let encoded = session.encode();
        let decoded = Session::from_str(&encoded).unwrap();
        assert_eq!(session.user_id, decoded.user_id);
        assert_eq!(session.username, decoded.username);
        assert_eq!(session.timestamp, decoded.timestamp);
    }
}
