//! The main command executer that starts the async runtime and runs the
//! respective subcommand
//!

use crate::cli::*;
use crate::config;
use crate::config::CONFIG;
use crate::controller::Controller;
use crate::controller::Gate;
use crate::discord;
use crate::server;
use crate::storage::{InMemoryStorage, SledUnencryptedStorage, Storage};
use chacha20poly1305::{
    aead::{KeyInit, OsRng},
    ChaCha20Poly1305,
};
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
            let storage = SledUnencryptedStorage::new();
            storage
                .list_guilds()
                .skip(*start as usize)
                .take(*end as usize - *start as usize)
                .for_each(|g| {
                    println!("{}", g);
                });
        }

        Some(Commands::Storage(StorageCmd::Guild(GuildCmd::Remove { guild_id }))) => {
            let mut storage = SledUnencryptedStorage::new();
            storage.remove_guild(*guild_id);
        }

        Some(Commands::Storage(StorageCmd::User(UserCmd::List { start, end }))) => {
            let storage = SledUnencryptedStorage::new();
            storage
                .list_users()
                .skip(*start as usize)
                .take(*end as usize - *start as usize)
                .for_each(|user| {
                    println!("{}: {}", user.0, user.1);
                });
        }

        Some(Commands::Storage(StorageCmd::User(UserCmd::Add {
            user_id,
            wallet_address,
        }))) => {
            let mut storage = SledUnencryptedStorage::new();
            storage.add_user(*user_id, wallet_address.to_string());
        }

        Some(Commands::Storage(StorageCmd::User(UserCmd::Remove { user_id }))) => {
            let mut storage = SledUnencryptedStorage::new();
            storage.remove_user(user_id);
        }

        Some(Commands::Storage(StorageCmd::Gate(GateCmd::List {
            guild,
            start,
            end,
            all_guilds,
        }))) => {
            let storage = SledUnencryptedStorage::new();
            let guilds = if *all_guilds {
                storage.list_guilds().collect::<Vec<u64>>()
            } else {
                if let Some(guild) = guild {
                    vec![*guild]
                } else {
                    vec![]
                }
            };
            for guild in guilds {
                println!("\nGuild: {}", guild);
                storage
                    .get_gates(&guild)
                    .skip(*start as usize)
                    .take(*end as usize - *start as usize)
                    .for_each(|gate| {
                        println!("{:?}", gate);
                    });
            }
        }
        Some(Commands::Storage(StorageCmd::Gate(GateCmd::Add {
            guild_id,
            colony_address,
            domain_id,
            reputation,
            role_id,
        }))) => {
            let mut storage = SledUnencryptedStorage::new();
            let gate = Gate {
                colony: colony_address.to_string(),
                domain: *domain_id,
                reputation: *reputation,
                role_id: *role_id,
            };
            storage.add_gate(guild_id, gate);
        }
        Some(Commands::Storage(StorageCmd::Gate(GateCmd::Remove {
            guild_id,
            colony_address,
            domain_id,
            reputation,
            role_id,
        }))) => {
            let gate = Gate {
                colony: colony_address.to_string(),
                domain: *domain_id,
                reputation: *reputation,
                role_id: *role_id,
            };
            let mut storage = SledUnencryptedStorage::new();
            storage.remove_gate(guild_id, gate);
        }

        Some(Commands::Discord(DiscordCmd::Register(RegisterCmd::Global))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(discord::register_global_slash_commands())
        }
        Some(Commands::Discord(DiscordCmd::Register(RegisterCmd::Guild { guild_id }))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(discord::register_guild_slash_commands(*guild_id));
        }
        Some(Commands::Discord(DiscordCmd::Delete(DeleteCmd::Global))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(discord::delete_global_slash_commands())
        }
        Some(Commands::Discord(DiscordCmd::Delete(DeleteCmd::Guild { guild_id }))) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(discord::delete_guild_slash_commands(*guild_id));
        }
        None => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            match CONFIG.wait().storage.storage_type {
                StorageType::Unencrypted => {
                    info!("Using unencrypted storage");
                    rt.spawn(Controller::<SledUnencryptedStorage>::init())
                }
                StorageType::InMemory => {
                    info!("Using in-memory storage");
                    rt.spawn(Controller::<InMemoryStorage>::init())
                }
                StorageType::Encrypted => todo!("Implement encrypted storage"),
            };
            rt.spawn(discord::start());
            if let Err(err) = rt.block_on(server::start()) {
                eprintln!("Error: {}", err);
            }
        }
    }
}
