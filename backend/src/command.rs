//! The main command executer that starts the async runtime and runs the
//! respective subcommand
//!

use crate::cli::*;
use crate::config;
use crate::config::CONFIG;
use crate::controller::Controller;
use crate::discord;
use crate::server;
use crate::storage::{InMemoryStorage, SledUnencryptedStorage};
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
        Some(Commands::Discord(DiscordCmd::Global)) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(discord::register_global_slash_commands())
        }
        Some(Commands::Discord(DiscordCmd::Server { guild_id })) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(discord::register_guild_slash_commands(*guild_id));
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
