//! # The colony discord gating bot
//!
//! By default the bot will store all data encrypted in an embedded database.
//! Most of the action will happen from slash commands in discord and the
//! following redirects to the http server.
//!
//! The bot can be configured via a config file, environment variables or
//! command line arguments.
//!
//! Other sub commands are used offline to help with certain
//! operations, e.g. key generation and most importantly the slash command
//! registration.
//!
//! ## First time usage
//! Before the bot can be used with discord, you need to setup a discord
//! application (and a bot) via the
//! [discord developer portal](https://discord.com/developers/applications).
//!
//!
//! When running the bot for the first time, no slash commands are
//! registered for the discord application, which makes the bot pretty useless.
//! With the `discord global/server` sub command, the bot will register all
//! slash commands either globally or for a specific guild. Global registration
//! may take some time to propagate, while guild registration is instant.
//!
//! To get started just run and go from there
//!```bash
//! discord-gating-bot help   
//!```
//! also man pages are genarated by the cargo build inside the man folder

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::missing_doc_code_examples)]

mod cli;
mod command;
mod config;
mod controller;
mod discord;
mod logging;
mod server;
mod storage;
use clap::Parser;
use cli::Cli;
use tracing::{instrument, warn};

#[cfg(feature = "completion")]
#[allow(unused_imports)]
use {clap::CommandFactory, clap_complete::generate, std::io};

/// The main entry point of the cli application. It sets up the logging and
/// configuration and then executes the command via the command module.
#[instrument(level = "trace")]
fn main() {
    let cli = Cli::parse();
    config::setup_config(&cli.cfg).expect("Failed to setup config");
    logging::setup_logging();
    command::execute(&cli);
}
