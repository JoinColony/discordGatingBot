//! Defines the storage trait for persisting data and holds different
//! implementations of it
//!

use crate::config::CONFIG;
use crate::controller::Gate;
use sled::{self, IVec};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};
use tracing::debug;

pub trait Storage {
    type GateIter: Iterator<Item = Gate>;
    type UserIter: Iterator<Item = (u64, String)>;
    type GuildIter: Iterator<Item = u64>;
    fn new() -> Self;
    fn list_guilds(&self) -> Self::GuildIter;
    fn remove_guild(&mut self, guild_id: u64);
    fn add_gate(&mut self, guild_id: &u64, gate: Gate);
    fn get_gates(&self, guild_id: &u64) -> Self::GateIter;
    fn remove_gate(&mut self, guild_id: &u64, gate: Gate);
    fn get_user(&self, user_id: &u64) -> Option<String>;
    fn list_users(&self) -> Self::UserIter;
    fn add_user(&mut self, user_id: u64, wallet: String);
    fn contains_user(&self, user_id: &u64) -> bool;
    fn remove_user(&mut self, user_id: &u64);
}

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

    fn remove_guild(&mut self, guild_id: u64) {
        self.gates.remove(&guild_id);
    }

    fn add_gate(&mut self, guild_id: &u64, gate: Gate) {
        self.gates.entry(*guild_id).or_default().push(gate);
    }

    fn remove_gate(&mut self, guild_id: &u64, gate: Gate) {
        let mut gates = self.gates.get(guild_id).unwrap().clone();
        gates.retain(|g| g != &gate);
        self.gates.insert(*guild_id, gates);
    }

    fn get_gates(&self, guild_id: &u64) -> Self::GateIter {
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
    fn add_user(&mut self, user_id: u64, wallet: String) {
        self.users.insert(user_id, wallet);
    }

    fn contains_user(&self, user_id: &u64) -> bool {
        self.users.contains_key(user_id)
    }

    fn remove_user(&mut self, user_id: &u64) {
        self.users.remove(user_id);
    }
}

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

    fn remove_guild(&mut self, guild_id: u64) {
        let tree_name = guild_id.to_be_bytes().to_vec();
        self.db.drop_tree(tree_name).unwrap();
    }
    fn add_gate(&mut self, guild_id: &u64, gate: Gate) {
        let tree = self.db.open_tree(guild_id.to_be_bytes()).unwrap();
        let gate_bytes = bincode::serialize(&gate).unwrap();
        let mut h = DefaultHasher::new();
        gate.hash(&mut h);
        tree.insert(h.finish().to_be_bytes(), gate_bytes).unwrap();
    }

    fn remove_gate(&mut self, guild_id: &u64, gate: Gate) {
        let tree = self.db.open_tree(guild_id.to_be_bytes()).unwrap();
        let mut h = DefaultHasher::new();
        gate.hash(&mut h);
        tree.remove(h.finish().to_be_bytes()).unwrap();
    }

    fn get_gates(&self, guild_id: &u64) -> Self::GateIter {
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

    fn add_user(&mut self, user_id: u64, wallet: String) {
        self.db
            .insert(user_id.to_be_bytes(), bincode::serialize(&wallet).unwrap())
            .unwrap();
    }

    fn contains_user(&self, user_id: &u64) -> bool {
        self.db.contains_key(user_id.to_be_bytes()).unwrap()
    }

    fn remove_user(&mut self, user_id: &u64) {
        self.db.remove(user_id.to_be_bytes()).unwrap();
    }
}
