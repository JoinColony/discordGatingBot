//! The main command executer that starts the async runtime and runs the
//! respective subcommand
//!

use crate::cli::*;

use crate::config;
use crate::config::CONFIG;
use crate::controller::{self, BatchResponse, Controller, Message};
use crate::discord;
use crate::gate::{Gate, ReputationGate, PRECISION_FACTOR};
use crate::server;
use crate::storage::{InMemoryStorage, SledEncryptedStorage, SledUnencryptedStorage, Storage};
use chacha20poly1305::{
    aead::{KeyInit, OsRng},
    ChaCha20Poly1305,
};
use colony_rs::U256;
use colony_rs::{u256_from_f64_saturating, H160};
use std::str::FromStr;
use tokio;
use tracing::{error, info};
#[cfg(feature = "completion")]
use {clap::CommandFactory, clap_complete::generate, std::io};

/// Chooses the appropriate actions based on the Commands enum
pub fn execute(cli: &Cli) {
    match &cli.cmd {
        #[cfg(feature = "completion")]
        Some(Commands::Completion { shell }) => {
            debug!("Generating completion script for {}", shell);
            let mut cmd = Cli::command();
            let cmd_name = cmd.get_name().to_string();
            generate(*shell, &mut cmd, cmd_name, &mut io::stdout());
        }

        Some(Commands::Config(ConfigCmd::Show)) => config::print_config(&cli.cfg),

        Some(Commands::Config(ConfigCmd::Template)) => config::print_template(),

        Some(Commands::Storage(StorageCmd::Generate)) => {
            let key = ChaCha20Poly1305::generate_key(&mut OsRng);
            println!("{}", hex::encode(key));
        }

        Some(Commands::Storage(StorageCmd::Guild(GuildCmd::List { start, end }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let storage = SledUnencryptedStorage::new();
                    storage
                        .list_guilds()
                        .skip(*start as usize)
                        .take(*end as usize - *start as usize)
                        .for_each(|g| {
                            println!("{}", g);
                        });
                }
                StorageType::Encrypted => {
                    let storage = SledEncryptedStorage::new();
                    storage
                        .list_guilds()
                        .skip(*start as usize)
                        .take(*end as usize - *start as usize)
                        .for_each(|g| {
                            println!("{}", g);
                        });
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::Guild(GuildCmd::Remove { guild_id }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let mut storage = SledUnencryptedStorage::new();
                    storage.remove_guild(*guild_id);
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage.remove_guild(*guild_id);
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::User(UserCmd::List { start, end }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let storage = SledUnencryptedStorage::new();
                    storage
                        .list_users()
                        .skip(*start as usize)
                        .take(*end as usize - *start as usize)
                        .for_each(|user| {
                            println!("{}: {}", user.0, user.1);
                        });
                }
                StorageType::Encrypted => {
                    let storage = SledEncryptedStorage::new();
                    storage
                        .list_users()
                        .skip(*start as usize)
                        .take(*end as usize - *start as usize)
                        .for_each(|user| {
                            println!("{}: {}", user.0, user.1);
                        });
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::User(UserCmd::Add {
            user_id,
            wallet_address,
        }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let mut storage = SledUnencryptedStorage::new();
                    storage.add_user(*user_id, wallet_address.to_string());
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage.add_user(*user_id, wallet_address.to_string());
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::User(UserCmd::Remove { user_id }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let mut storage = SledUnencryptedStorage::new();
                    storage.remove_user(user_id);
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage.remove_user(user_id);
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::Gate(GateCmd::List { guild, start, end }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let storage = SledUnencryptedStorage::new();
                    let guilds = if let Some(guild) = guild {
                        vec![*guild]
                    } else {
                        storage.list_guilds().collect::<Vec<u64>>()
                    };
                    for guild in guilds {
                        println!("\nGuild: {}", guild);
                        storage
                            .list_gates(&guild)
                            .skip(*start as usize)
                            .take(*end as usize - *start as usize)
                            .for_each(|gate| {
                                println!("{:?}", gate);
                            });
                    }
                }
                StorageType::Encrypted => {
                    let storage = SledEncryptedStorage::new();
                    let guilds = if let Some(guild) = guild {
                        vec![*guild]
                    } else {
                        storage.list_guilds().collect::<Vec<u64>>()
                    };
                    for guild in guilds {
                        println!("\nGuild: {}", guild);
                        storage
                            .list_gates(&guild)
                            .skip(*start as usize)
                            .take(*end as usize - *start as usize)
                            .for_each(|gate| {
                                println!("{:?}", gate);
                            });
                    }
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::Gate(GateCmd::Add {
            guild_id,
            colony_address,
            domain_id,
            reputation,
            role_id,
        }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let mut storage = SledUnencryptedStorage::new();
                    let colony = H160::from_str(colony_address).expect("Invalid colony address");
                    let gate = Gate {
                        role_id: *role_id,
                        condition: Box::new(ReputationGate {
                            chain_id: U256::from(100),
                            colony_address: colony,
                            colony_name: "".to_string(),
                            colony_domain: *domain_id,
                            reputation_threshold_scaled: u256_from_f64_saturating(
                                *reputation * PRECISION_FACTOR,
                            ),
                        }),
                    };
                    storage.add_gate(guild_id, gate);
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    let colony = H160::from_str(colony_address).expect("Invalid colony address");
                    let gate = Gate {
                        role_id: *role_id,
                        condition: Box::new(ReputationGate {
                            chain_id: U256::from(100),
                            colony_address: colony,
                            colony_name: "".to_string(),
                            colony_domain: *domain_id,
                            reputation_threshold_scaled: u256_from_f64_saturating(
                                *reputation * PRECISION_FACTOR,
                            ),
                        }),
                    };
                    storage.add_gate(guild_id, gate);
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::Gate(GateCmd::Remove {
            guild_id,
            colony_address,
            domain_id,
            reputation,
            role_id,
        }))) => {
            let colony = H160::from_str(colony_address).expect("Invalid colony address");
            let gate = Gate {
                role_id: *role_id,
                condition: Box::new(ReputationGate {
                    chain_id: U256::from(100),
                    colony_address: colony,
                    colony_domain: *domain_id,
                    colony_name: "".to_string(),
                    reputation_threshold_scaled: u256_from_f64_saturating(
                        *reputation * PRECISION_FACTOR,
                    ),
                }),
            };
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let mut storage = SledUnencryptedStorage::new();
                    storage.remove_gate(guild_id, gate);
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage.remove_gate(guild_id, gate);
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Discord(DiscordCmd::Register(RegisterCmd::Global))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            rt.block_on(discord::register_global_slash_commands())
        }

        Some(Commands::Discord(DiscordCmd::Register(RegisterCmd::Guild { guild_id }))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            rt.block_on(discord::register_guild_slash_commands(*guild_id));
        }

        Some(Commands::Discord(DiscordCmd::Delete(DeleteCmd::Global))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            rt.block_on(discord::delete_global_slash_commands())
        }

        Some(Commands::Discord(DiscordCmd::Delete(DeleteCmd::Guild { guild_id }))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            rt.block_on(discord::delete_guild_slash_commands(*guild_id));
        }

        Some(Commands::Check { guild_id, user_id }) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            let controller: Controller<SledEncryptedStorage> = Controller::new();
            let user_result = controller.storage.get_user(&user_id);
            let gates = controller.storage.list_gates(&guild_id);
            let wallet = match user_result {
                Some(wallet) => wallet,
                None => return error!("User not found"),
            };
            let roles = rt.block_on(controller::check_user(wallet, gates));
            println!("Roles: {:?}", roles);
        }

        Some(Commands::Batch { guild_id, user_ids }) => {
            let guild_id = *guild_id;
            let user_ids = user_ids.clone();
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            let controller: Controller<SledEncryptedStorage> = Controller::new();
            let message_tx = controller.message_tx.clone();
            rt.spawn(controller.spawn());
            let (response_tx, mut response_rx) = tokio::sync::mpsc::channel(100);
            rt.spawn(async move {
                message_tx
                    .send(Message::Batch {
                        guild_id,
                        user_ids,
                        response_tx,
                    })
                    .await
                    .expect("Failed to send batch message to controller");
            });
            rt.block_on(async move {
                while let Some(response) = response_rx.recv().await {
                    match response {
                        BatchResponse::Grant { user_id, roles } => {
                            println!("User: {}, Roles: {:?}", user_id, roles);
                        }
                        BatchResponse::Done => {
                            println!("Done");
                            break;
                        }
                    }
                }
            });
        }
        None => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    info!("Using unencrypted storage");
                    rt.spawn(Controller::<SledUnencryptedStorage>::init())
                }
                StorageType::InMemory => {
                    info!("Using in-memory storage");
                    rt.spawn(Controller::<InMemoryStorage>::init())
                }
                StorageType::Encrypted => {
                    info!("Using encrypted storage");
                    rt.spawn(Controller::<SledEncryptedStorage>::init())
                }
            };
            rt.spawn(discord::start());
            if let Err(err) = rt.block_on(server::start()) {
                eprintln!("Error: {}", err);
            }
        }
    }
}
