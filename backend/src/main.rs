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

#[cfg(feature = "build_info")]
#[allow(unused_imports)]
use shadow_rs::shadow;
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

/// Prints the build information gathered at compile time.
#[cfg(feature = "build_info")]
fn print_build_info() {
    println!("project_name:         {}", build::PROJECT_NAME);
    println!("pkg_version:          {}", build::PKG_VERSION);

    println!("\n#git");
    println!("commit_id:            {}", build::COMMIT_HASH);
    println!("tag:                  {}", build::TAG);
    println!("branch:               {}", build::BRANCH);
    println!("commit_date:          {}", build::COMMIT_DATE);
    println!("commit_author:        {}", build::COMMIT_AUTHOR);
    println!("commit_email:         {}", build::COMMIT_EMAIL);

    println!("\n#build");
    println!("build_os:             {}", build::BUILD_OS);
    println!("rust_version:         {}", build::RUST_VERSION);
    println!("rust_channel:         {}", build::RUST_CHANNEL);
    println!("cargo_version:        {}", build::CARGO_VERSION);
    println!("build_time:           {}", build::BUILD_TIME);
    println!("build_rust_channel:   {}", build::BUILD_RUST_CHANNEL);
}
