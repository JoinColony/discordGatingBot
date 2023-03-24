//! This is the main busines logic of the application. It registers a global
//! static channel on which other parts of the application can communicate with
//! the controller.
//!

use crate::gate::Gate;
use crate::{config::CONFIG, storage::Storage};
use anyhow::{anyhow, bail, Error, Result};
use chacha20poly1305::{
    aead::{
        generic_array::GenericArray,
        {Aead, AeadCore, KeyInit, OsRng},
    },
    ChaCha20Poly1305,
};
use colony_rs::H160;
use futures::FutureExt;
use hex;
use once_cell::sync::OnceCell;
use secrecy::{ExposeSecret, SecretString};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::Mutex,
    sync::{mpsc, oneshot},
    task::JoinSet,
};
use tracing::{debug, error, info, info_span, instrument, Instrument, Span};
use urlencoding;

/// The global channel on which the controller can be communicated with
pub static CONTROLLER_CHANNEL: OnceCell<mpsc::Sender<Message>> = OnceCell::new();
/// A session encryption key which is used to encrypt the session used for
/// user registration. It is generated once at startup and never changes as
/// long as the application is running.
static SESSION_KEY: OnceCell<Vec<u8>> = OnceCell::new();

/// The message type is the main way for other parts of the application to
/// communicate with the controller.
#[derive(Debug)]
pub enum Message {
    List {
        guild_id: u64,
        response: oneshot::Sender<Vec<Gate>>,
        span: Span,
    },
    Roles {
        guild_id: u64,
        response: oneshot::Sender<HashSet<u64>>,
        span: Span,
    },
    Delete {
        guild_id: u64,
        gate: Gate,
        span: Span,
    },
    Gate {
        guild_id: u64,
        gate: Gate,
        span: Span,
    },
    Check {
        guild_id: u64,
        user_id: u64,
        username: String,
        response_tx: oneshot::Sender<CheckResponse>,
        span: Span,
    },
    Batch {
        guild_id: u64,
        user_ids: Vec<u64>,
        response_tx: mpsc::Sender<BatchResponse>,
        span: Span,
    },
    Register {
        user_id: u64,
        wallet: SecretString,
        response_tx: oneshot::Sender<RegisterResponse>,
        span: Span,
    },
    Unregister {
        user_id: u64,
        username: String,
        response_tx: oneshot::Sender<UnRegisterResponse>,
        removed_tx: oneshot::Sender<RemoveUserResponse>,
        span: Span,
    },
    RemovUser {
        session: String,
        response_tx: oneshot::Sender<RemoveUserResponse>,
        span: Span,
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

#[derive(Debug)]
pub enum RemoveUserResponse {
    Success,
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
        tokio::spawn(self.controller_loop());
    }

    async fn controller_loop(mut self)
    where
        S: Storage + Send + 'static,
        <S as Storage>::GateIter: Send,
    {
        let pending_unregisters: Arc<Mutex<HashMap<String, oneshot::Sender<RemoveUserResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        while let Some(message) = self.message_rx.recv().await {
            match message {
                Message::Gate {
                    guild_id,
                    gate,
                    span,
                } => self.add_gate(guild_id, gate, span).await,
                Message::Roles {
                    guild_id,
                    response,
                    span,
                } => self.list_roles(guild_id, response, span),
                Message::List {
                    guild_id,
                    response,
                    span,
                } => self.list_gates(guild_id, response, span),
                Message::Delete {
                    guild_id,
                    gate,
                    span,
                } => self.delete_gate(guild_id, gate, span),
                Message::Check {
                    username,
                    user_id,
                    guild_id,
                    response_tx,
                    span,
                } => {
                    self.check(guild_id, username, user_id, response_tx, span)
                        .await
                }
                Message::Batch {
                    guild_id,
                    user_ids,
                    response_tx,
                    span,
                } => {
                    self.batch_check(guild_id, user_ids, response_tx, span)
                        .await
                }
                Message::Register {
                    user_id,
                    wallet,
                    response_tx,
                    span,
                } => self.register(user_id, wallet, response_tx, span).await,
                Message::Unregister {
                    username,
                    user_id,
                    response_tx,
                    removed_tx,
                    span,
                } => {
                    self.unregister(
                        username,
                        user_id,
                        response_tx,
                        removed_tx,
                        pending_unregisters.clone(),
                        span,
                    )
                    .await
                }
                Message::RemovUser {
                    session,
                    response_tx,
                    span,
                } => {
                    self.delete_user(session, response_tx, pending_unregisters.clone(), span)
                        .await
                }
            }
        }
    }

    async fn add_gate(&mut self, guild_id: u64, gate: Gate, span: Span) {
        let _enter = span.enter();
        debug!(?gate, "Adding gate:");
        if let Err(why) = self.storage.add_gate(&guild_id, gate) {
            error!("Failed to add gate: {:?}", why);
        }
    }

    fn list_roles(&mut self, guild_id: u64, response: oneshot::Sender<HashSet<u64>>, span: Span) {
        let _enter = span.enter();
        match self.storage.list_gates(&guild_id) {
            Ok(gates) => {
                let roles = HashSet::from_iter(gates.into_iter().map(|gate| gate.role_id));
                if let Err(why) = response.send(roles) {
                    error!("Failed to send roles: {:?}", why);
                }
            }
            Err(why) => {
                error!("Failed to list gates: {:?}", why);
            }
        }
    }

    fn list_gates(&mut self, guild_id: u64, response: oneshot::Sender<Vec<Gate>>, span: Span) {
        let _enter = span.enter();
        debug!("Received list request for guild");
        match self.storage.list_gates(&guild_id) {
            Ok(gate_iter) => {
                let gates = gate_iter.collect::<Vec<Gate>>();
                debug!(?gates, "Sending list response");
                if let Err(why) = response.send(gates) {
                    error!("Failed to send list response: {:?}", why);
                }
            }
            Err(why) => {
                error!("Failed to list gates: {:?}", why);
            }
        }
    }

    fn delete_gate(&mut self, guild_id: u64, gate: Gate, span: Span) {
        let _enter = span.enter();
        debug!("Deleting gate: {:?}", gate);
        if let Err(why) = self.storage.remove_gate(&guild_id, gate.identifier()) {
            error!("Failed to delete gate: {:?}", why);
        }
    }

    async fn check(
        &mut self,
        guild_id: u64,
        username: String,
        user_id: u64,
        response_tx: oneshot::Sender<CheckResponse>,
        span: Span,
    ) {
        let _enter = span.enter();
        debug!("Checking user");
        if !self.storage.contains_user(&user_id) {
            debug!("User not registered");
            let url = CONFIG.wait().server.url.clone();
            let session = match Session::new(user_id, username) {
                Ok(session) => session,
                Err(why) => {
                    error!("Failed to create session: {:?}", why);
                    if let Err(why) = response_tx.send(CheckResponse::Error(why)) {
                        error!("Failed to send register response: {:?}", why);
                    }
                    return;
                }
            };

            let encoded_session = match session.encode() {
                Ok(session) => session,
                Err(why) => {
                    error!("Failed to encode session: {:?}", why);
                    if let Err(why) = response_tx.send(CheckResponse::Error(why)) {
                        error!("Failed to send register response: {:?}", why);
                    }
                    return;
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
            return;
        }

        let wallet = match self.storage.get_user(&user_id) {
            Ok(wallet) => wallet,
            Err(why) => {
                error!("Failed to get user: {:?}", why);
                if let Err(why) = response_tx.send(CheckResponse::Error(why)) {
                    error!("Failed to send CheckResponse::Error: {:?}", why);
                }
                return;
            }
        };
        match self.storage.list_gates(&guild_id) {
            Err(why) => {
                error!("Failed to list gates: {:?}", why);
                if let Err(why) = response_tx.send(CheckResponse::Error(why)) {
                    error!("Failed to send error response: {:?}", why);
                }
            }
            Ok(gates) => {
                debug!("Found wallet for user");
                let granted_roles = check_with_wallet(wallet, gates).in_current_span().await;
                let _guard = span.enter();
                debug!(?granted_roles, "Roles granted");
                if let Err(why) = response_tx.send(CheckResponse::Grant(granted_roles)) {
                    error!("Failed to send CheckResponse::Grant: {:?}", why);
                };
            }
        }
    }

    async fn batch_check(
        &mut self,
        guild_id: u64,
        user_ids: Vec<u64>,
        response_tx: mpsc::Sender<BatchResponse>,
        span: Span,
    ) where
        S: Storage + Send + 'static,
        <S as Storage>::GateIter: Send,
    {
        let _enter = span.enter();
        debug!(?user_ids, "Batch checking");
        let check_futures = user_ids
            .into_iter()
            .filter(|user_id| self.storage.contains_user(user_id))
            .filter_map(|user_id| match self.storage.get_user(&user_id) {
                Ok(wallet) => Some((user_id, wallet)),
                Err(why) => {
                    error!("Failed to get user: {:?}", why);
                    return None;
                }
            })
            .filter_map(
                |(user_id, wallet)| match self.storage.list_gates(&guild_id) {
                    Ok(gates) => Some(
                        check_with_wallet(wallet, gates)
                            .map(move |granted_roles| (user_id, granted_roles)),
                    ),
                    Err(why) => {
                        error!("Failed to list gates: {:?}", why);
                        None
                    }
                },
            );
        let mut set = JoinSet::new();
        for fut in check_futures {
            set.spawn(fut.in_current_span());
        }
        let timeout = Duration::from_millis(CONFIG.wait().internal_timeout);
        while let Some(result) = set.join_next().in_current_span().await {
            let _enter = span.enter();
            match result {
                Ok((user_id, roles)) => {
                    debug!(user_id, ?roles, "Batch result");
                    if let Err(why) = response_tx
                        .send_timeout(BatchResponse::Grant { user_id, roles }, timeout)
                        .in_current_span()
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
        if let Err(why) = response_tx
            .send_timeout(BatchResponse::Done, timeout)
            .in_current_span()
            .await
        {
            error!("Failed to send BatchResponse::Done: {:?}", why);
        };
    }

    async fn register(
        &mut self,
        user_id: u64,
        wallet: SecretString,
        response_tx: oneshot::Sender<RegisterResponse>,
        span: Span,
    ) {
        let _enter = span.enter();
        debug!("Registering user {} with wallet {:?}", user_id, wallet);
        if self.storage.contains_user(&user_id) {
            debug!("User {} already registered", user_id);
            if let Err(why) = response_tx.send(RegisterResponse::AlreadyRegistered) {
                error!(
                    "Failed to send RegisterResponse::AlreadyRegistered: {:?}",
                    why
                );
            };
        } else {
            if let Err(why) = self.storage.add_user(user_id, wallet) {
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

    async fn unregister(
        &mut self,
        username: String,
        user_id: u64,
        response_tx: oneshot::Sender<UnRegisterResponse>,
        removed_tx: oneshot::Sender<RemoveUserResponse>,
        pending_unregisters: Arc<Mutex<HashMap<String, oneshot::Sender<RemoveUserResponse>>>>,
        span: Span,
    ) {
        let _enter = span.enter();
        debug!("Unregistering user");
        if !self.storage.contains_user(&user_id) {
            if let Err(why) = response_tx.send(UnRegisterResponse::NotRegistered) {
                error!(
                    "Failed to send UnregisterResponse::NotRegistered: {:?}",
                    why
                );
            };
            return;
        }
        let url = CONFIG.wait().server.url.clone();
        let session = match Session::new(user_id, username) {
            Ok(session) => session,
            Err(why) => {
                error!("Failed to create session: {:?}", why);
                if let Err(why) = response_tx.send(UnRegisterResponse::Error(why)) {
                    error!("Failed to send UnregisterResponse::Error: {:?}", why);
                };
                return;
            }
        };
        let encoded_session = match session.encode() {
            Ok(encoded) => encoded,
            Err(why) => {
                error!("Failed to encode session: {:?}", why);
                if let Err(why) = response_tx.send(UnRegisterResponse::Error(why)) {
                    error!("Failed to send UnregisterResponse::Error: {:?}", why);
                };
                return;
            }
        };
        debug!(?session, ?encoded_session, "Created session");
        let url = format!(
            "{}/unregister/{}/{}",
            url,
            urlencoding::encode(&session.username),
            encoded_session,
        );
        if let Err(why) = response_tx.send(UnRegisterResponse::Unregister(url)) {
            error!("Failed to send CheckResponse::Register: {:?}", why);
        };
        let pending_unregisters2 = pending_unregisters.clone();
        let esession = encoded_session.clone();
        let mut guard = pending_unregisters.lock().in_current_span().await;
        tokio::spawn(async move {
            let span = info_span!("unregister_timeout");
            let expiration = CONFIG.wait().session_expiration;
            tokio::time::sleep(std::time::Duration::from_secs(expiration))
                .in_current_span()
                .await;
            let _enter = span.enter();
            info!("Session expired");
            let mut guard = pending_unregisters2.lock().in_current_span().await;
            if let Some(removed_tx) = guard.remove(&esession) {
                if let Err(why) =
                    removed_tx.send(RemoveUserResponse::Error(anyhow!("Session expired")))
                {
                    error!("Failed to send RemoveUserResponse::Expired: {:?}", why);
                };
            }
        });
        guard.insert(encoded_session, removed_tx);
    }

    async fn delete_user(
        &mut self,
        session_str: String,
        response_tx: oneshot::Sender<RemoveUserResponse>,
        pending_unregisters: Arc<Mutex<HashMap<String, oneshot::Sender<RemoveUserResponse>>>>,
        span: Span,
    ) {
        let _enter = span.enter();
        let session = match Session::from_str(&session_str) {
            Ok(session) => session,
            Err(why) => {
                error!("Failed to decode session: {:?}", why);
                if let Err(why) = response_tx.send(RemoveUserResponse::Error(why)) {
                    error!("Failed to send RemoveUserResponse::Error: {:?}", why);
                };
                return;
            }
        };
        let mut guard = pending_unregisters.lock().in_current_span().await;
        let removed_tx = guard.remove(&session_str);
        if session.expired() {
            error!(?session, "Session expired");
            if let Err(why) =
                response_tx.send(RemoveUserResponse::Error(anyhow!("Session expired")))
            {
                error!("Failed to send RemoveUserResponse::Error: {:?}", why);
            };
            if let Some(removed_tx) = removed_tx {
                if let Err(why) =
                    removed_tx.send(RemoveUserResponse::Error(anyhow!("Session expired")))
                {
                    error!("Failed to send RemoveUserResponse::Success: {:?}", why);
                };
            } else {
                error!("No pending unregister for session {}", session_str);
            }
            return;
        }
        debug!(session.user_id, "Removing user");
        if let Err(why) = self.storage.remove_user(&session.user_id) {
            error!("Failed to remove user: {:?}", why);
        }
        if let Err(why) = response_tx.send(RemoveUserResponse::Success) {
            error!("Failed to send RemoveUserResponse::Success: {:?}", why);
        };
        if let Some(removed_tx) = removed_tx {
            if let Err(why) = removed_tx.send(RemoveUserResponse::Success) {
                error!("Failed to send RemoveUserResponse::Success: {:?}", why);
            };
        } else {
            error!("No pending unregister for session {}", session_str);
        }
    }
}

#[instrument(level = "debug", skip(wallet, gates))]
pub async fn check_with_wallet(
    wallet: SecretString,
    gates: impl Iterator<Item = Gate>,
) -> Vec<u64> {
    debug!("Checking with the user's wallet");
    let wallet = match H160::from_str(&wallet.expose_secret()) {
        Ok(wallet) => wallet,
        Err(why) => {
            error!("Invalid wallet address: {:?}:{:?}", wallet, why);
            return Vec::new();
        }
    };
    let wallet_arc = Arc::new(wallet);
    let mut set = JoinSet::new();
    for gate in gates {
        debug!(
            name = gate.name(),
            gate.role_id,
            identifier = gate.identifier(),
            "Checking gate"
        );
        let wallet = wallet_arc.clone();
        set.spawn(gate.check_condition(*wallet).in_current_span());
    }
    let mut granted_roles = Vec::new();
    while let Some(check_result) = set.join_next().in_current_span().await {
        match check_result {
            Ok(result) => match result {
                Some(role_id) => granted_roles.push(role_id),
                None => debug!("Gate did not grant a role"),
            },
            Err(why) => {
                error!("Failed to check gate: {:?}", why);
            }
        }
    }
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
    pub fn new(user_id: u64, username: String) -> Result<Self> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        Ok(Session {
            user_id,
            username,
            timestamp,
        })
    }

    pub fn expired(&self) -> bool {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get system timestamp")
            .as_secs();
        timestamp - self.timestamp > CONFIG.wait().session_expiration
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
        let session = Session::new(123, "test".to_string()).unwrap();
        let encoded = session.encode().unwrap();
        let decoded = Session::from_str(&encoded).unwrap();
        assert_eq!(session.user_id, decoded.user_id);
        assert_eq!(session.username, decoded.username);
        assert_eq!(session.timestamp, decoded.timestamp);
    }
}
