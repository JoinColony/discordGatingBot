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

pub trait Storage {
    type GateIter: Iterator<Item = Gate>;
    fn new() -> Self;
    fn add_gate(&mut self, guild_id: &u64, gate: Gate);
    fn get_gates(&self, guild_id: &u64) -> Self::GateIter;
    fn get_user(&self, user_id: &u64) -> Option<String>;
    fn add_user(&mut self, user_id: u64, wallet: String);
    fn contains_user(&self, user_id: &u64) -> bool;
}

pub struct InMemoryStorage {
    gates: HashMap<u64, Vec<Gate>>,
    users: HashMap<u64, String>,
}

impl Storage for InMemoryStorage {
    type GateIter = std::vec::IntoIter<Gate>;
    fn new() -> Self {
        InMemoryStorage {
            gates: HashMap::new(),
            users: HashMap::new(),
        }
    }
    fn add_gate(&mut self, guild_id: &u64, gate: Gate) {
        self.gates.entry(*guild_id).or_default().push(gate);
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

    fn add_user(&mut self, user_id: u64, wallet: String) {
        self.users.insert(user_id, wallet);
    }

    fn contains_user(&self, user_id: &u64) -> bool {
        self.users.contains_key(user_id)
    }
}

pub struct SledUnencryptedStorage {
    db: sled::Db,
}

impl Storage for SledUnencryptedStorage {
    type GateIter = std::iter::Map<sled::Iter, fn(Result<(IVec, IVec), sled::Error>) -> Gate>;

    fn new() -> Self {
        let db_path = &CONFIG.wait().storage.directory;
        let db = sled::open(db_path).expect("Failed to open database");
        SledUnencryptedStorage { db }
    }

    fn add_gate(&mut self, guild_id: &u64, gate: Gate) {
        let tree = self.db.open_tree(guild_id.to_be_bytes()).unwrap();
        let gate_bytes = bincode::serialize(&gate).unwrap();
        let mut h = DefaultHasher::new();
        gate.hash(&mut h);
        tree.insert(h.finish().to_be_bytes(), gate_bytes).unwrap();
    }

    fn get_gates(&self, guild_id: &u64) -> Self::GateIter {
        let tree = self.db.open_tree(guild_id.to_be_bytes()).unwrap();
        tree.iter()
            .map(|x| bincode::deserialize::<Gate>(&x.unwrap().1).unwrap())
    }

    fn get_user(&self, user_id: &u64) -> Option<String> {
        let wallet = self.db.get(user_id.to_string()).unwrap();
        if let Some(wallet) = wallet {
            let wallet: String = bincode::deserialize(&wallet).unwrap();
            Some(wallet)
        } else {
            None
        }
    }

    fn add_user(&mut self, user_id: u64, wallet: String) {
        self.db
            .insert(user_id.to_string(), bincode::serialize(&wallet).unwrap())
            .unwrap();
    }

    fn contains_user(&self, user_id: &u64) -> bool {
        self.db.contains_key(user_id.to_string()).unwrap()
    }
}
