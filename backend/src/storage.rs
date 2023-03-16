//! Defines the storage trait for persisting data and holds different
//! implementations of it
//!

use crate::config::CONFIG;
use crate::gate::Gate;
use anyhow::{anyhow, bail, Result};
use chacha20poly1305::{
    aead::generic_array::GenericArray,
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use hex;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use sled::{self, IVec};
use std::collections::HashMap;
use tracing::{debug, error};

/// The storage trait that defines the methods that need to be implemented
/// for a storage backend
pub trait Storage {
    type GateIter: Iterator<Item = Gate>;
    type UserIter: Iterator<Item = (u64, String)>;
    type GuildIter: Iterator<Item = u64>;
    fn new() -> Self;
    fn list_guilds(&self) -> Self::GuildIter;
    fn remove_guild(&mut self, guild_id: u64) -> Result<()>;
    fn add_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()>;
    fn list_gates(&self, guild_id: &u64) -> Self::GateIter;
    fn remove_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()>;
    fn get_user(&self, user_id: &u64) -> Option<String>;
    fn list_users(&self) -> Self::UserIter;
    fn add_user(&mut self, user_id: u64, wallet: String) -> Result<()>;
    fn contains_user(&self, user_id: &u64) -> bool;
    fn remove_user(&mut self, user_id: &u64) -> Result<()>;
}

/// The in-memory storage backend which does not persist data to disk
/// should only be used for testing
pub struct InMemoryStorage {
    gates: HashMap<u64, Vec<Gate>>,
    users: HashMap<u64, String>,
}

impl Storage for InMemoryStorage {
    type GateIter = std::vec::IntoIter<Gate>;
    type UserIter = std::collections::hash_map::IntoIter<u64, String>;
    type GuildIter = std::collections::hash_map::IntoKeys<u64, Vec<Gate>>;

    fn new() -> Self {
        InMemoryStorage {
            gates: HashMap::new(),
            users: HashMap::new(),
        }
    }
    fn list_guilds(&self) -> Self::GuildIter {
        self.gates.clone().into_keys()
    }

    fn remove_guild(&mut self, guild_id: u64) -> Result<()> {
        self.gates
            .remove(&guild_id)
            .ok_or(anyhow!("guild {} does not exist", guild_id))?;
        Ok(())
    }

    fn add_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()> {
        self.gates.entry(*guild_id).or_default().push(gate);
        Ok(())
    }

    fn remove_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()> {
        let mut gates = match self.gates.get(guild_id) {
            Some(gates) => gates.clone(),
            None => {
                error!("No gates found for guild {}", guild_id);
                bail!("No gates found for guild {}", guild_id);
            }
        };
        gates.retain(|g| g != &gate);
        self.gates.insert(*guild_id, gates);
        Ok(())
    }

    fn list_gates(&self, guild_id: &u64) -> Self::GateIter {
        if let Some(gates) = self.gates.get(&guild_id) {
            gates.clone().into_iter()
        } else {
            Vec::new().into_iter()
        }
    }

    fn get_user(&self, user_id: &u64) -> Option<String> {
        self.users.get(user_id).cloned()
    }

    fn list_users(&self) -> Self::UserIter {
        self.users.clone().into_iter()
    }
    fn add_user(&mut self, user_id: u64, wallet: String) -> Result<()> {
        self.users.insert(user_id, wallet);
        Ok(())
    }

    fn contains_user(&self, user_id: &u64) -> bool {
        self.users.contains_key(user_id)
    }

    fn remove_user(&mut self, user_id: &u64) -> Result<()> {
        self.users
            .remove(user_id)
            .ok_or(anyhow!("user {} does not exist", user_id))?;
        Ok(())
    }
}

/// The sled storage backend which persists data to disk unencrypted
pub struct SledUnencryptedStorage {
    db: sled::Db,
}

impl Storage for SledUnencryptedStorage {
    type GateIter = std::iter::Map<sled::Iter, fn(Result<(IVec, IVec), sled::Error>) -> Gate>;
    type UserIter =
        std::iter::Map<sled::Iter, fn(Result<(IVec, IVec), sled::Error>) -> (u64, String)>;
    type GuildIter = std::iter::FilterMap<std::vec::IntoIter<IVec>, fn(IVec) -> Option<u64>>;

    fn new() -> Self {
        let db_path = &CONFIG.wait().storage.directory;
        let db = sled::open(db_path).expect("Failed to open database");
        SledUnencryptedStorage { db }
    }

    fn list_guilds(&self) -> Self::GuildIter {
        self.db.tree_names().into_iter().filter_map(|t| {
            if let Ok(bytes) = t.to_vec().try_into() {
                Some(u64::from_be_bytes(bytes))
            } else {
                None
            }
        })
    }

    fn remove_guild(&mut self, guild_id: u64) -> Result<()> {
        let tree_name = guild_id.to_be_bytes().to_vec();
        self.db.drop_tree(tree_name)?;
        Ok(())
    }

    fn add_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()> {
        let tree = self.db.open_tree(guild_id.to_be_bytes())?;
        let gate_bytes = bincode::serialize(&gate)?;
        let key = gate.identifier();
        debug!("Adding gate {:?} with key {}", gate, key);
        tree.insert(key.to_be_bytes(), gate_bytes)?;
        Ok(())
    }

    fn remove_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()> {
        let tree = self.db.open_tree(guild_id.to_be_bytes())?;
        let key = gate.identifier();
        debug!("Removing gate {:?} with key {}", gate, key);
        tree.remove(key.to_be_bytes())?;
        Ok(())
    }

    fn list_gates(&self, guild_id: &u64) -> Self::GateIter {
        let tree = self.db.open_tree(guild_id.to_be_bytes()).unwrap();
        tree.iter()
            .map(|x| bincode::deserialize::<Gate>(&x.unwrap().1).unwrap())
    }

    fn get_user(&self, user_id: &u64) -> Option<String> {
        let wallet = self.db.get(user_id.to_be_bytes()).unwrap();
        if let Some(wallet) = wallet {
            let wallet: String = bincode::deserialize(&wallet).unwrap();
            Some(wallet)
        } else {
            None
        }
    }

    fn list_users(&self) -> Self::UserIter {
        self.db.iter().map(|x| {
            let (key, value) = x.unwrap();
            debug!("key: {:?}, value: {:?}", key, value);
            let key: u64 = u64::from_be_bytes(key.to_vec().try_into().unwrap());
            let value: String = bincode::deserialize(&value).unwrap();
            (key, value)
        })
    }

    fn add_user(&mut self, user_id: u64, wallet: String) -> Result<()> {
        self.db
            .insert(user_id.to_be_bytes(), bincode::serialize(&wallet)?)?;
        Ok(())
    }

    fn contains_user(&self, user_id: &u64) -> bool {
        self.db.contains_key(user_id.to_be_bytes()).unwrap_or(false)
    }

    fn remove_user(&mut self, user_id: &u64) -> Result<()> {
        self.db.remove(user_id.to_be_bytes())?;
        Ok(())
    }
}

/// The default sled storage backend which persists data to disk and encrypts
/// the wallet addresses of users
pub struct SledEncryptedStorage {
    db: sled::Db,
}

impl Storage for SledEncryptedStorage {
    type GateIter = std::iter::Map<sled::Iter, fn(Result<(IVec, IVec), sled::Error>) -> Gate>;
    type UserIter =
        std::iter::Map<sled::Iter, fn(Result<(IVec, IVec), sled::Error>) -> (u64, String)>;
    type GuildIter = std::iter::FilterMap<std::vec::IntoIter<IVec>, fn(IVec) -> Option<u64>>;

    fn new() -> Self {
        let db_path = &CONFIG.wait().storage.directory;
        let db = sled::open(db_path).expect("Failed to open database");
        Self { db }
    }

    fn list_guilds(&self) -> Self::GuildIter {
        self.db.tree_names().into_iter().filter_map(|t| {
            if let Ok(bytes) = t.to_vec().try_into() {
                Some(u64::from_be_bytes(bytes))
            } else {
                None
            }
        })
    }

    fn remove_guild(&mut self, guild_id: u64) -> Result<()> {
        let tree_name = guild_id.to_be_bytes().to_vec();
        self.db.drop_tree(tree_name)?;
        Ok(())
    }

    fn add_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()> {
        let tree = self.db.open_tree(guild_id.to_be_bytes())?;
        let gate_bytes = bincode::serialize(&gate)?;
        let key = gate.identifier();
        tree.insert(key.to_be_bytes(), gate_bytes)?;
        Ok(())
    }

    fn remove_gate(&mut self, guild_id: &u64, gate: Gate) -> Result<()> {
        let tree = self.db.open_tree(guild_id.to_be_bytes())?;
        let key = gate.identifier();
        tree.remove(key.to_be_bytes())?;
        Ok(())
    }

    fn list_gates(&self, guild_id: &u64) -> Self::GateIter {
        let tree = self.db.open_tree(guild_id.to_be_bytes()).unwrap();
        tree.iter()
            .map(|x| bincode::deserialize::<Gate>(&x.unwrap().1).unwrap())
    }

    fn get_user(&self, user_id: &u64) -> Option<String> {
        let wallet = self.db.get(user_id.to_be_bytes()).unwrap();
        if let Some(wallet) = wallet {
            let encrypted: EncryptionWrapper = bincode::deserialize(&wallet).unwrap();
            Some(encrypted.decrypt())
        } else {
            None
        }
    }

    fn list_users(&self) -> Self::UserIter {
        self.db.iter().map(|x| {
            let (key, value) = x.unwrap();
            debug!("key: {:?}, value: {:?}", key, value);
            let key: u64 = u64::from_be_bytes(key.to_vec().try_into().unwrap());
            let encrypted: EncryptionWrapper = bincode::deserialize(&value).unwrap();
            let decrypted = encrypted.decrypt();
            (key, decrypted)
        })
    }

    fn add_user(&mut self, user_id: u64, wallet: String) -> Result<()> {
        let encrypted = EncryptionWrapper::new(wallet);
        self.db.insert(
            user_id.to_be_bytes(),
            bincode::serialize(&encrypted).unwrap(),
        )?;
        Ok(())
    }

    fn contains_user(&self, user_id: &u64) -> bool {
        self.db.contains_key(user_id.to_be_bytes()).unwrap()
    }

    fn remove_user(&mut self, user_id: &u64) -> Result<()> {
        self.db.remove(user_id.to_be_bytes())?;
        Ok(())
    }
}

/// A convinience wrapper around the stored user wallet addresses, that
/// also holds the nonce used for encryption
#[derive(Debug, Serialize, Deserialize)]
struct EncryptionWrapper {
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

impl EncryptionWrapper {
    fn new(plaintext: String) -> Self {
        let key_hex = &CONFIG.wait().storage.key.expose_secret();
        let key_bytes = hex::decode(key_hex).unwrap();
        let key = GenericArray::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .expect("encryption failure!");
        Self {
            nonce: nonce.to_vec(),
            ciphertext: ciphertext.to_vec(),
        }
    }

    fn decrypt(&self) -> String {
        let key_hex = &CONFIG.wait().storage.key.expose_secret();
        let key_bytes = hex::decode(key_hex).unwrap();
        let key = GenericArray::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = GenericArray::from_slice(&self.nonce);
        let plaintext = cipher.decrypt(&nonce, self.ciphertext.as_ref()).unwrap();
        String::from_utf8(plaintext).unwrap()
    }
}
