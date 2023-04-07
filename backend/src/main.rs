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

#![deny(missing_docs)]
#![deny(rustdoc::missing_crate_level_docs)]
#![deny(rustdoc::missing_doc_code_examples)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(rustdoc::private_doc_tests)]
#![deny(rustdoc::invalid_codeblock_attributes)]
#![deny(rustdoc::invalid_html_tags)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![deny(rustdoc::invalid_html_tags)]

mod cli;
mod command;
mod config;
mod controller;
mod discord;
mod gate;
mod logging;
mod server;
mod storage;
use clap::Parser;
use cli::Cli;
use tracing::{instrument, warn};

/// The main entry point of the cli application. It sets up the logging and
/// configuration and then executes the command via the command module.
#[instrument]
fn main() {
    #[cfg(feature = "profiling")]
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .expect("Failed to start profiler");
    let cli = Cli::parse();
    // for certain commands we need to skip the config setup
    match cli.cmd {
        Some(cli::Commands::Storage(cli::StorageCmd::Generate)) => {}
        Some(cli::Commands::Config(_)) => {}
        _ => {
            config::setup_config(&cli.cfg).expect("Failed to setup config");
            logging::setup_logging();
        }
    }
    command::execute(&cli);
    #[cfg(feature = "profiling")]
    if let Ok(report) = guard.report().build() {
        let file =
            std::fs::File::create("flamegraph.svg").expect("Failed to create file for flamegraph");
        report.flamegraph(file).expect("Failed to write flamegraph");
    };
}
