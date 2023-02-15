//! This is the main business logic of the application. It registers a global
//! static channel on which other parts of the application can communicate with
//! the controller.
//!

use crate::{config::CONFIG, storage::Storage};
use anyhow::{bail, Result};
use chacha20poly1305::{
    aead::generic_array::GenericArray,
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use hex;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot};
use tracing::error;

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
    Gate {
        colony: String,
        reputation: u32,
        role_id: u64,
        guild_id: u64,
    },
    Check {
        user_id: u64,
        guild_id: u64,
        response_tx: oneshot::Sender<CheckResponse>,
    },
    Register {
        user_id: u64,
        wallet: String,
        response_tx: oneshot::Sender<RegisterResponse>,
    },
}

/// The response to a check message, sent back via the oneshot channel in the
/// inbound message.
#[derive(Debug)]
pub enum CheckResponse {
    NoGates,
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

/// Represents a gate for a discord role issues by the /gate slash command.
/// This is stored in the database for each discord server.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Gate {
    /// The colony address in which the reputation should be looked up
    colony: String,
    /// The reputation amount required to be granted the role
    reputation: u32,
    /// The role to be granted
    role_id: u64,
}

/// The main business logic instance. It holds a storage instance and a channel
/// for communication with other parts of the application.
#[derive(Debug)]
pub struct Controller<S: Storage> {
    storage: S,
    message_tx: mpsc::Sender<Message>,
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

    /// Starts the controller and sets the global static channel for other
    /// parts of the application to communicate with it.
    pub async fn init() {
        let key = ChaCha20Poly1305::generate_key(&mut OsRng);
        SESSION_KEY
            .set(key.to_vec())
            .expect("Failed to set session key");
        let mut controller: Controller<S> = Controller::new();
        CONTROLLER_CHANNEL
            .set(controller.message_tx.clone())
            .expect("Failed to set controller channel");
        tokio::spawn(async move {
            while let Some(message) = controller.message_rx.recv().await {
                match message {
                    Message::Gate {
                        colony,
                        reputation,
                        role_id,
                        guild_id,
                    } => {
                        let gate = Gate {
                            colony,
                            reputation,
                            role_id,
                        };
                        controller.storage.add_gate(&guild_id, gate);
                    }
                    Message::Check {
                        user_id,
                        guild_id,
                        response_tx,
                    } => {
                        if let Some(wallet) = controller.storage.get_user(&user_id) {
                            if let Some(gates) = controller.storage.get_gates(&guild_id) {
                                let granted_roles: Vec<_> = gates
                                    .iter()
                                    .filter(|gate| {
                                        let reputation = check_reputation(&gate.colony, &wallet);
                                        reputation >= gate.reputation
                                    })
                                    .map(|gate| gate.role_id)
                                    .collect();
                                if let Err(why) =
                                    response_tx.send(CheckResponse::Grant(granted_roles))
                                {
                                    error!("Failed to send CheckResponse::Grant: {:?}", why);
                                };
                            } else {
                                if let Err(why) = response_tx.send(CheckResponse::NoGates) {
                                    error!("Failed to send CheckResponse::NoGates: {:?}", why);
                                };
                            }
                        } else {
                            let url = CONFIG.wait().server.url.clone();
                            let port = CONFIG.wait().server.port;
                            let session = Session::new(user_id);
                            let url = format!("{}:{}/session/{}", url, port, session.encode());
                            if let Err(why) = response_tx.send(CheckResponse::Register(url)) {
                                error!("Failed to send CheckResponse::Register: {:?}", why);
                            };
                        }
                    }
                    Message::Register {
                        user_id,
                        wallet,
                        response_tx,
                    } => {
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
                }
            }
        });
    }
}

/// This is used to gather the fraction of total reputation a wallet has in
/// a domain in a colony
fn check_reputation(colony: &str, wallet: &str) -> u32 {
    match (colony, wallet) {
        ("colony1", "wallet1") => 10,
        ("colony1", "wallet2") => 20,
        ("colony2", "wallet2") => 30,
        ("colony2", "wallet1") => 40,
        _ => 0,
    }
}

/// This represents a session for a user that has not yet registered their
/// and is used to generate a url for the user to register their wallet.
/// The session is encoded as a nonce and string separated by a dot.
/// The string is an encrypted version of the user information
#[derive(Debug, Deserialize, Serialize)]
pub struct Session {
    pub user_id: u64,
    pub timestamp: u64,
}

impl Session {
    pub fn new(user_id: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get system timestamp")
            .as_secs();
        Session { user_id, timestamp }
    }

    pub fn expired(&self) -> bool {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get system timestamp")
            .as_secs();
        timestamp - self.timestamp > 60
    }

    pub fn encode(&self) -> String {
        let plaintext_str = format!("{}:{}", self.user_id, self.timestamp);

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
        let user_id: u64;
        let timestamp;

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
        if parts.len() != 2 {
            bail!("Invalid session string");
        }
        user_id = parts[0].parse()?;
        timestamp = parts[1].parse()?;

        Ok(Self { user_id, timestamp })
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
        let session = Session::new(123);
        let encoded = session.encode();
        let decoded = Session::from_str(&encoded).unwrap();
        assert_eq!(session.user_id, decoded.user_id);
        assert_eq!(session.timestamp, decoded.timestamp);
    }
}
