use crate::cli::*;
use crate::config;
use crate::controller::Controller;
use crate::discord;
use crate::server;
use crate::storage;
use chacha20poly1305::{
    aead::{KeyInit, OsRng},
    ChaCha20Poly1305,
};
#[cfg(feature = "completion")]
use clap::CommandFactory;
#[cfg(feature = "completion")]
use clap_complete::generate;
#[cfg(feature = "build_info")]
use shadow_rs::shadow;
#[cfg(feature = "completion")]
use std::io;
use tokio;
use tracing::info;

pub fn execute(cli: &Cli) {
    match &cli.cmd {
        #[cfg(feature = "completion")]
        Some(Commands::Completion { shell }) => {
            info!("Generating completion script for {}", shell);
            let mut cmd = Cli::command();
            let cmd_name = cmd.get_name().to_string();
            generate(*shell, &mut cmd, cmd_name, &mut io::stdout());
        }
        Some(Commands::Config(ConfigCmd::Show)) => config::print_config(&cli.cfg),
        Some(Commands::Config(ConfigCmd::Template)) => config::print_template(),
        Some(Commands::Key(CryptCmd::Generate)) => {
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
            rt.spawn(Controller::<storage::SledUnencryptedStorage>::init());
            rt.spawn(discord::start());
            if let Err(err) = rt.block_on(server::start()) {
                eprintln!("Error: {}", err);
            }
        }
    }
}
