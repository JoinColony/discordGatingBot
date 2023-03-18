//! This is the main busines logic of the application. It registers a global
//! static channel on which other parts of the application can communicate with
//! the controller.
//!

use crate::gate::Gate;
use crate::{config::CONFIG, storage::Storage};
use anyhow::{anyhow, bail, Error, Result};
use chacha20poly1305::{
    aead::generic_array::GenericArray,
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use colony_rs::H160;
use futures::FutureExt;
use hex;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use std::{
    collections::HashSet,
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
    Roles {
        guild_id: u64,
        response: oneshot::Sender<HashSet<u64>>,
    },
    Delete {
        guild_id: u64,
        gate: Gate,
    },
    Gate {
        guild_id: u64,
        gate: Gate,
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
        response_tx: mpsc::Sender<BatchResponse>,
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
    Error(Error),
}

#[derive(Debug)]
pub enum BatchResponse {
    Grant { user_id: u64, roles: Vec<u64> },
    Done,
}

/// The response to a register message, sent back via the oneshot channel in the
/// inbound message.
#[derive(Debug)]
pub enum RegisterResponse {
    AlreadyRegistered,
    Success,
    Error(Error),
}

/// The response to a unregister message, sent back via the oneshot channel in the
/// inbound message.
#[derive(Debug)]
pub enum UnRegisterResponse {
    NotRegistered,
    Unregister(String),
    Error(Error),
}

/// The main business logic instance. It holds a storage instance and a channel
/// for communication with other parts of the application.
#[derive(Debug)]
pub struct Controller<S: Storage> {
    pub storage: S,
    pub message_tx: mpsc::Sender<Message>,
    message_rx: mpsc::Receiver<Message>,
}

impl<S: Storage + Send + 'static + std::marker::Sync> Controller<S> {
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
                    Message::Gate { guild_id, gate } => {
                        debug!("Adding gate: {:?}", gate);
                        if let Err(why) = controller.storage.add_gate(&guild_id, gate) {
                            error!("Failed to add gate: {:?}", why);
                        }
                    }
                    Message::Roles { guild_id, response } => {
                        match controller.storage.list_gates(&guild_id) {
                            Ok(gates) => {
                                let roles =
                                    HashSet::from_iter(gates.into_iter().map(|gate| gate.role_id));
                                if let Err(why) = response.send(roles) {
                                    error!("Failed to send roles: {:?}", why);
                                }
                            }
                            Err(why) => {
                                error!("Failed to list gates: {:?}", why);
                            }
                        }
                    }
                    Message::List { guild_id, response } => {
                        debug!("Received list request for guild {}", guild_id);
                        match controller.storage.list_gates(&guild_id) {
                            Ok(gate_iter) => {
                                let gates = gate_iter.collect::<Vec<Gate>>();
                                debug!("Sending list response gates {:?}", gates);
                                if let Err(why) = response.send(gates) {
                                    error!("Failed to send list response: {:?}", why);
                                }
                            }
                            Err(why) => {
                                error!("Failed to list gates: {:?}", why);
                            }
                        }
                    }
                    Message::Delete { guild_id, gate } => {
                        debug!("Deleting gate: {:?}", gate);
                        if let Err(why) = controller.storage.remove_gate(&guild_id, gate) {
                            error!("Failed to delete gate: {:?}", why);
                        }
                    }
                    Message::Check {
                        user_id,
                        username,
                        guild_id,
                        response_tx,
                    } => {
                        debug!("Checking user {} in guild {}", user_id, guild_id);
                        if !controller.storage.contains_user(&user_id) {
                            let url = CONFIG.wait().server.url.clone();
                            let session = Session::new(user_id, username);
                            let encoded_session = match session.encode() {
                                Ok(session) => session,
                                Err(why) => {
                                    error!("Failed to encode session: {:?}", why);
                                    if let Err(why) = response_tx.send(CheckResponse::Error(why)) {
                                        error!("Failed to send register response: {:?}", why);
                                    }
                                    continue;
                                }
                            };
                            let url = format!(
                                "{}/register/{}/{}",
                                url,
                                urlencoding::encode(&session.username),
                                encoded_session
                            );
                            if let Err(why) = response_tx.send(CheckResponse::Register(url)) {
                                error!("Failed to send CheckResponse::Register: {:?}", why);
                            };
                            continue;
                        }

                        let wallet = match controller.storage.get_user(&user_id) {
                            Ok(wallet) => wallet,
                            Err(why) => {
                                error!("Failed to get user: {:?}", why);
                                if let Err(why) = response_tx.send(CheckResponse::Error(why)) {
                                    error!("Failed to send CheckResponse::Error: {:?}", why);
                                }
                                continue;
                            }
                        };
                        match controller.storage.list_gates(&guild_id) {
                            Err(why) => {
                                error!("Failed to list gates: {:?}", why);
                                if let Err(why) = response_tx.send(CheckResponse::Error(why)) {
                                    error!("Failed to send error response: {:?}", why);
                                }
                            }
                            Ok(gates) => {
                                debug!("User {} has wallet {}", user_id, wallet);
                                let granted_roles = check_user(wallet, gates).await;
                                debug!("Granted roles deduped: {:?}", granted_roles);
                                if let Err(why) =
                                    response_tx.send(CheckResponse::Grant(granted_roles))
                                {
                                    error!("Failed to send CheckResponse::Grant: {:?}", why);
                                };
                            }
                        }
                    }
                    Message::Batch {
                        guild_id,
                        user_ids,
                        response_tx,
                    } => {
                        debug!("Batch checking users {:?} in guild {}", user_ids, guild_id);
                        let check_futures = user_ids
                            .into_iter()
                            .filter_map(|user_id| match controller.storage.get_user(&user_id) {
                                Ok(wallet) => Some((user_id, wallet)),
                                Err(why) => {
                                    error!("Failed to get user: {:?}", why);
                                    return None;
                                }
                            })
                            .filter_map(|(user_id, wallet)| {
                                match controller.storage.list_gates(&guild_id) {
                                    Ok(gates) => Some(
                                        check_user(wallet, gates)
                                            .map(move |granted_roles| (user_id, granted_roles)),
                                    ),
                                    Err(why) => {
                                        error!("Failed to list gates: {:?}", why);
                                        None
                                    }
                                }
                            });
                        let mut set = JoinSet::new();
                        for fut in check_futures {
                            set.spawn(fut);
                        }
                        while let Some(result) = set.join_next().await {
                            debug!("Batch checking {:?}", result);
                            match result {
                                Ok((user_id, roles)) => {
                                    debug!("User {} granted roles {:?}", user_id, roles);
                                    if let Err(why) = response_tx
                                        .send(BatchResponse::Grant { user_id, roles })
                                        .await
                                    {
                                        error!("Failed to send BatchResponse::Grant: {:?}", why);
                                    };
                                }
                                Err(why) => {
                                    error!("Failed to check user: {:?}", why);
                                }
                            }
                        }
                        debug!("Batch check complete, sending done");
                        if let Err(why) = response_tx.send(BatchResponse::Done).await {
                            error!("Failed to send BatchResponse::Done: {:?}", why);
                        };
                    }
                    Message::Register {
                        user_id,
                        wallet,
                        response_tx,
                    } => {
                        debug!("Registering user {} with wallet {}", user_id, wallet);
                        if controller.storage.contains_user(&user_id) {
                            debug!("User {} already registered", user_id);
                            if let Err(why) = response_tx.send(RegisterResponse::AlreadyRegistered)
                            {
                                error!(
                                    "Failed to send RegisterResponse::AlreadyRegistered: {:?}",
                                    why
                                );
                            };
                        } else {
                            if let Err(why) = controller.storage.add_user(user_id, wallet) {
                                error!("Failed to add user: {:?}", why);
                                if let Err(why) = response_tx.send(RegisterResponse::Error(why)) {
                                    error!("Failed to send RegisterResponse::Error: {:?}", why);
                                };
                            } else {
                                if let Err(why) = response_tx.send(RegisterResponse::Success) {
                                    error!("Failed to send RegisterResponse::Success: {:?}", why);
                                };
                            }
                        }
                    }
                    Message::Unregister {
                        user_id,
                        username,
                        response_tx,
                    } => {
                        debug!("Unregistering user {}", user_id);
                        if !controller.storage.contains_user(&user_id) {
                            if let Err(why) = response_tx.send(UnRegisterResponse::NotRegistered) {
                                error!(
                                    "Failed to send UnregisterResponse::NotRegistered: {:?}",
                                    why
                                );
                            };
                            continue;
                        }

                        match controller.storage.get_user(&user_id) {
                            Err(why) => {
                                error!("Failed to get user: {:?}", why);
                                if let Err(why) = response_tx.send(UnRegisterResponse::Error(why)) {
                                    error!("Failed to send UnregisterResponse::Error: {:?}", why);
                                };
                            }
                            Ok(_) => {
                                let url = CONFIG.wait().server.url.clone();
                                let session = Session::new(user_id, username);
                                let encoded_session = match session.encode() {
                                    Ok(encoded) => encoded,
                                    Err(why) => {
                                        error!("Failed to encode session: {:?}", why);
                                        if let Err(why) =
                                            response_tx.send(UnRegisterResponse::Error(why))
                                        {
                                            error!(
                                                "Failed to send UnregisterResponse::Error: {:?}",
                                                why
                                            );
                                        };
                                        continue;
                                    }
                                };
                                let url = format!(
                                    "{}/unregister/{}/{}",
                                    url,
                                    urlencoding::encode(&session.username),
                                    encoded_session,
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
                        if let Err(why) = controller.storage.remove_user(&user_id) {
                            error!("Failed to remove user: {:?}", why);
                        }
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
        let wallet = if let Ok(wallet) = H160::from_str(&wallet) {
            wallet
        } else {
            error!("Invalid wallet address: {} ", wallet);
            continue;
        };
        set.spawn(gate.check(wallet));
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
    granted_roles
}

/// This represents a session for a user that has not yet registered their
/// and is used to generate a url for the user to register their wallet.
/// The session is encoded as a nonce and string separated by a dot.
/// The string is an encrypted version of the user information
#[derive(Debug)]
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

    pub fn encode(&self) -> Result<String> {
        let plaintext_str = format!("{}:{}:{}", self.user_id, self.username, self.timestamp);

        let plaintext = plaintext_str.as_bytes();
        let key_bytes = SESSION_KEY.wait();
        let key = GenericArray::from_slice(&key_bytes);

        let cipher = ChaCha20Poly1305::new(key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow!("{e}"))?;
        let encoded_nonce = hex::encode(nonce);
        let encoded_ciphertext = hex::encode(ciphertext);
        Ok(format!("{}.{}", encoded_nonce, encoded_ciphertext))
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
        let encoded = session.encode().unwrap();
        let decoded = Session::from_str(&encoded).unwrap();
        assert_eq!(session.user_id, decoded.user_id);
        assert_eq!(session.username, decoded.username);
        assert_eq!(session.timestamp, decoded.timestamp);
    }
}
