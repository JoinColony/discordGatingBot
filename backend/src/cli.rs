/// The cli module defines all subcommands and sets up the cli parser
///
/// Additional commands can be added via the `Commands` enum
///
use clap::Args;
use clap::{
    crate_authors, crate_description, crate_name, crate_version, Parser, Subcommand, ValueHint,
};
#[cfg(feature = "completion")]
use clap_complete::Shell;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

static LONG_DESCRIPTION: Lazy<String> = Lazy::new(|| {
    include_str!("../README.tpl")
        .lines()
        .filter(|l| !(l.contains("{{") && l.contains("}}")))
        .collect::<Vec<&str>>()
        .join("\n")
});

/// `Cli` is the main struct for the cli parser, it contains the gloabl flags
/// and the `Commands` enum with all subcommands
#[derive(Parser)]
#[clap(
    name = crate_name!(),
    author = crate_authors!("\n"),
    version = crate_version!(),
    about = crate_description!(),
    long_about = LONG_DESCRIPTION.as_str(),
    arg_required_else_help = false
)]
pub struct Cli {
    #[cfg(feature = "build_info")]
    /// Print build information
    #[clap(long)]
    pub build_info: bool,
    /// The main configuration params live here, some of them can be set as global flags
    #[clap(flatten)]
    pub cfg: CliConfig,
    /// The subcommand to run
    #[clap(subcommand)]
    pub cmd: Option<Commands>,
}

/// The commands enum contains all sub commands and their respective arguments
#[derive(Subcommand)]
#[clap()]
pub enum Commands {
    /// Generates completion scripts for the specified shell
    #[cfg(feature = "completion")]
    Completion {
        #[clap(value_parser, action)]
        shell: Shell,
    },
    /// Print or edit the configuration
    #[clap(subcommand)]
    Config(ConfigCmd),
    /// Generate an encrypted key
    #[clap(subcommand)]
    Key(CryptCmd),
    /// Interact with discord directly
    #[clap(subcommand)]
    Discord(DiscordCmd),
}
#[derive(Subcommand)]
#[clap()]
pub enum ConfigCmd {
    /// Print the configuration sources and merged config
    Show,
    /// Prints an example configuration template
    Template,
}

#[derive(Subcommand)]
#[clap()]
pub enum DiscordCmd {
    /// Register the global slash commands
    Global,
    /// Register the slash commands for a specific guild
    Server {
        /// The guild id
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
    },
}

#[derive(Subcommand)]
#[clap()]
pub enum CryptCmd {
    /// Generates a new key than can be used for encryption at rest and for
    /// the sessions tokens
    Generate,
}

/// This structs contains the global configuration for the application
/// it is merged from the config file, the environment and the command line
/// arguments
#[derive(Args, Clone, Debug, Serialize, Deserialize)]
#[clap()]
pub struct CliConfig {
    /// Sets a custom config file
    #[clap(short, long,  value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub config_file: Option<PathBuf>,
    /// Define the verbosity of the application, repeat for more verbosity
    #[clap(long, short = 'v', global(true), parse(from_occurrences))]
    pub verbose: u8,
    #[clap(long, short, global(true), conflicts_with = "verbose")]
    /// Supress all logging
    pub quiet: bool,
    #[clap(flatten)]
    pub discord: CliDiscordConfig,
    #[clap(flatten)]
    pub server: CliServerConfig,
    #[clap(flatten)]
    pub acme: CliAcmeConfig,
    #[clap(flatten)]
    pub encryption: CliEncryptionConfig,
}

#[derive(Args, Clone, Debug, Serialize, Deserialize)]
#[clap()]
pub struct CliEncryptionConfig {
    /// The encryption key to use for the database and session tokens
    #[clap(long = "encryption-key")]
    pub encryption_key: Option<String>,
}

#[derive(Args, Clone, Debug, Serialize, Deserialize)]
#[clap()]
pub struct CliDiscordConfig {
    /// The discord bot token
    #[clap(short, long)]
    pub token: Option<String>,
    /// The number of guild shards
    #[clap(short, long)]
    pub shards: Option<u8>,
}

#[derive(Args, Clone, Debug, Serialize, Deserialize)]
#[clap()]
pub struct CliServerConfig {
    /// The address to listen on
    #[clap(short, long)]
    pub host: Option<String>,
    /// The port to listen on
    #[clap(short, long)]
    pub port: Option<u16>,
    /// The path to the certificate File
    #[clap( long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub cert: Option<PathBuf>,
    /// The path to the private key File
    #[clap(short, long, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub key: Option<PathBuf>,
}

#[derive(Args, Clone, Debug, Serialize, Deserialize)]
#[clap()]
pub struct CliAcmeConfig {
    /// The address of the acme server to use
    #[clap(long)]
    pub acme_endpoint: Option<String>,
    /// The port to listen on
    #[clap(long)]
    pub acme_port: Option<u16>,
    #[clap( long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    /// The path to the directory where the certificates are stored
    pub directory: Option<PathBuf>,
    /// The path to the directory where the certificates are stored
    #[clap( long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub staging_directory: Option<PathBuf>,
    /// The path to the directory where the certificates are stored
    #[clap(long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    pub staging: Option<bool>,
}
