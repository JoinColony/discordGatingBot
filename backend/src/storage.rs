//! Defines the storage trait for persisting data and holds different
//! implementations of it
//!

use crate::config::CONFIG;
use crate::controller::Gate;
use sled;
use std::collections::HashMap;

pub trait Storage {
    fn new() -> Self;
    fn add_gate(&mut self, guild_id: &u64, gate: Gate);
    fn get_gates(&self, guild_id: &u64) -> Option<Vec<Gate>>;
    fn get_user(&self, user_id: &u64) -> Option<String>;
    fn add_user(&mut self, user_id: u64, wallet: String);
    fn contains_user(&self, user_id: &u64) -> bool;
}

pub struct InMemoryStorage {
    gates: HashMap<u64, Vec<Gate>>,
    users: HashMap<u64, String>,
}

impl Storage for InMemoryStorage {
    fn new() -> Self {
        InMemoryStorage {
            gates: HashMap::new(),
            users: HashMap::new(),
        }
    }
    fn add_gate(&mut self, guild_id: &u64, gate: Gate) {
        self.gates.entry(*guild_id).or_default().push(gate);
    }

    fn get_gates(&self, guild_id: &u64) -> Option<Vec<Gate>> {
        self.gates.get(&guild_id).cloned()
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
    fn new() -> Self {
        let db_path = &CONFIG.wait().storage.directory;
        let db = sled::open(db_path).unwrap();
        SledUnencryptedStorage { db }
    }

    fn add_gate(&mut self, guild_id: &u64, gate: Gate) {
        let gates = self.get_gates(guild_id).unwrap_or(Vec::new()).clone();
        let mut gates = gates;
        gates.push(gate);
        self.db
            .insert(guild_id.to_string(), bincode::serialize(&gates).unwrap())
            .unwrap();
    }

    fn get_gates(&self, guild_id: &u64) -> Option<Vec<Gate>> {
        let gates = self.db.get(guild_id.to_string()).unwrap();
        if let Some(gates) = gates {
            let gates: Vec<Gate> = bincode::deserialize(&gates).unwrap();
            Some(gates)
        } else {
            None
        }
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
