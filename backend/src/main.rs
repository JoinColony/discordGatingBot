//! The colony discord gating bot.
//!
//!
//! To get started just run and go from there
//!```bash
//! discord-gating-bot help   
//!```

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

// This pulls in compile time information
#[cfg(feature = "build_info")]
shadow!(build);
/// The main entry point of the cli application. It parses the command line
/// flags and then calls the appropriate sub command choosen by the `Commands`
/// enum.
#[instrument(level = "trace")]
fn main() {
    let cli = Cli::parse();

    #[cfg(feature = "build_info")]
    if cli.build_info {
        print_build_info();
        return;
    }

    config::setup_config(&cli.cfg).expect("Failed to setup config");
    logging::setup_logging();
    command::execute(&cli);
}
