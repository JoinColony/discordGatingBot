//! The main command executer that starts the async runtime and runs the
//! respective subcommand
//!

use crate::cli::*;

use crate::config;
use crate::config::CONFIG;
use crate::controller::{self, BatchResponse, Controller, Message};
use crate::discord;
use crate::server;
use crate::storage::{InMemoryStorage, SledEncryptedStorage, SledUnencryptedStorage, Storage};
use chacha20poly1305::{
    aead::{KeyInit, OsRng},
    ChaCha20Poly1305,
};
use secrecy::ExposeSecret;
use tokio;
use tracing::info;
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
                    storage
                        .remove_guild(*guild_id)
                        .expect("Failed to remove guild");
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage
                        .remove_guild(*guild_id)
                        .expect("Failed to remove guild");
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
                        .expect("Failed to list users")
                        .skip(*start as usize)
                        .take(*end as usize - *start as usize)
                        .for_each(|user| {
                            println!("{}: {}", user.0, user.1.expose_secret());
                        });
                }
                StorageType::Encrypted => {
                    let storage = SledEncryptedStorage::new();
                    storage
                        .list_users()
                        .expect("Failed to list users")
                        .skip(*start as usize)
                        .take(*end as usize - *start as usize)
                        .for_each(|user| {
                            println!("{}: {}", user.0, user.1.expose_secret());
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
                    storage
                        .add_user(*user_id, wallet_address.to_string().into())
                        .expect("Failed to add user");
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage
                        .add_user(*user_id, wallet_address.to_string().into())
                        .expect("Failed to add user");
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
                    storage.remove_user(user_id).expect("Failed to remove user");
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage.remove_user(user_id).expect("Failed to remove user");
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
                            .expect("Failed to list gates")
                            .skip(*start as usize)
                            .take(*end as usize - *start as usize)
                            .for_each(|gate| {
                                println!("{}:{:?}", gate.identifier(), gate);
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
                            .expect("Failed to list gates")
                            .skip(*start as usize)
                            .take(*end as usize - *start as usize)
                            .for_each(|gate| {
                                println!("{}:{:?}", gate.identifier(), gate);
                            });
                    }
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Storage(StorageCmd::Gate(GateCmd::Remove {
            guild_id,
            identifier,
        }))) => {
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    let mut storage = SledUnencryptedStorage::new();
                    storage
                        .remove_gate(guild_id, *identifier)
                        .expect("Failed to remove gate");
                }
                StorageType::Encrypted => {
                    let mut storage = SledEncryptedStorage::new();
                    storage
                        .remove_gate(guild_id, *identifier)
                        .expect("Failed to remove gate");
                }
                StorageType::InMemory => {
                    panic!("InMemory storage does not make sense for this command")
                }
            };
        }

        Some(Commands::Slash(SlashCommands::Register(RegisterCmd::Global))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            rt.block_on(discord::register_global_slash_commands())
        }

        Some(Commands::Slash(SlashCommands::Register(RegisterCmd::Guild { guild_id }))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            rt.block_on(discord::register_guild_slash_commands(*guild_id));
        }

        Some(Commands::Slash(SlashCommands::Delete(DeleteCmd::Global))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
            rt.block_on(discord::delete_global_slash_commands())
        }

        Some(Commands::Slash(SlashCommands::Delete(DeleteCmd::Guild { guild_id }))) => {
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
            let wallet = controller
                .storage
                .get_user(&user_id)
                .expect("Failed to get user");
            let gates = controller
                .storage
                .list_gates(&guild_id)
                .expect("Failed to list gates");
            let roles = rt.block_on(controller::check_with_wallet(wallet, gates));
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
            let span = tracing::info_span!("Batch");
            rt.spawn(async move {
                message_tx
                    .send(Message::Batch {
                        guild_id,
                        user_ids,
                        response_tx,
                        span,
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
