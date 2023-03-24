//! The cli module defines all subcommands and sets up the cli parser
//!
//! Additional commands can be added via the `Commands` enum
//!
use clap::Args;
use clap::{
    crate_authors, crate_description, crate_name, crate_version, Parser, Subcommand, ValueHint,
};
use once_cell::sync::Lazy;
use secrecy::SecretString;
use serde::Deserialize;
use std::path::PathBuf;

/// The long description used for the help subcommand and man page.
/// It is sourced from the cargo readme template, removing all template
/// expression from the file.
static LONG_DESCRIPTION: Lazy<String> = Lazy::new(|| {
    format!(
        "{}\n{}",
        crate_description!(),
        include_str!("../README.tpl")
            .lines()
            .filter(|l| !(l.contains("{{") && l.contains("}}")))
            .collect::<Vec<&str>>()
            .join("\n")
    )
});

/// `Cli` is the main struct for the cli parser, it contains the gloabl flags
/// and the `Commands` enum with all subcommands
#[derive(Debug, Parser)]
#[clap(
    name = crate_name!(),
    author = crate_authors!("\n"),
    version = crate_version!(),
    about = crate_description!(),
    long_about = LONG_DESCRIPTION.as_str(),
    arg_required_else_help = false
)]
pub struct Cli {
    /// The main configuration params live here and can be set with command line
    /// flags
    #[clap(flatten)]
    pub cfg: CliConfig,
    /// The subcommand to run
    #[clap(subcommand)]
    pub cmd: Option<Commands>,
}

/// The commands enum contains all sub commands and their respective arguments
#[derive(Debug, Subcommand)]
#[clap()]
pub enum Commands {
    /// Print the configuration and get a template file
    #[clap(subcommand)]
    Config(ConfigCmd),
    /// Interact with the presistent storage and encryption
    #[clap(subcommand)]
    Storage(StorageCmd),
    /// Interact with the discord slash commands
    #[clap(subcommand)]
    Slash(SlashCommands),
    /// Perfom a check on a user as if this user would have used the
    /// `/get in` slash command in that guild
    Check {
        /// The guild id in which the user should be checked
        guild_id: u64,
        /// The discord user id to check
        user_id: u64,
    },
    /// Perfom a check on a batch of user as if this someone on the server
    /// would have used the `/gate enforce` slash command
    Batch {
        /// The guild id in which the users should be checked
        guild_id: u64,
        /// The discord user ids to check
        user_ids: Vec<u64>,
    },
}

/// Represents the config sub command, used to print the current config or get a template
#[derive(Debug, Subcommand)]
#[clap()]
pub enum ConfigCmd {
    /// Print the configuration sources and merged config
    Show,
    /// Prints an example configuration template
    Template,
}

/// Represents the slashcommands sub command, used to register and delete slash commands
#[derive(Debug, Subcommand)]
#[clap()]
pub enum SlashCommands {
    /// Register the global slash commands
    #[clap(subcommand)]
    Register(RegisterCmd),
    /// Register the slash commands for a specific guild
    #[clap(subcommand)]
    Delete(DeleteCmd),
}

/// represents the discord sub command, used to register slash commands in
/// a specific guild or globally
#[derive(Debug, Subcommand)]
#[clap()]
pub enum RegisterCmd {
    /// Register the global slash commands
    Global,
    /// Register the slash commands for a specific guild
    Guild {
        /// The guild id in which the commands should be registered
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
    },
}

/// represents the discord sub command, used to delete slash commands in
/// a specific guild or globally
#[derive(Debug, Subcommand)]
#[clap()]
pub enum DeleteCmd {
    /// Delete the global slash commands
    Global,
    /// Delete the slash commands in a specific guild
    Guild {
        /// The guild id in which the commands should be deleted
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
    },
}

/// Represents the storage sub command, used to interact with the stored data
/// and encryption. Commands that use the data on disk, only work if the
/// bot is not running, otherwise the data is locked.
/// Be careful, these commands are able to alter data in the storage_type
/// and also expose secretes to the console, especially the user commands
#[derive(Debug, Subcommand)]
#[clap()]
pub enum StorageCmd {
    /// Generates a new key than can be used for encryption at rest
    Generate,
    /// List or delete discord guilds in the db
    #[clap(subcommand)]
    Guild(GuildCmd),
    #[clap(subcommand)]
    /// List, add or delete discord users in the db
    User(UserCmd),
    #[clap(subcommand)]
    /// List, add or delete discord role gates in the db
    Gate(GateCmd),
}

/// Represents the user sub command, used to interact with the user storage
#[derive(Debug, Subcommand)]
#[clap()]
pub enum GuildCmd {
    /// List all guilds
    List {
        /// Starting index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "0")]
        start: u64,
        /// End index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "100")]
        end: u64,
    },
    /// Remove a guild
    Remove {
        /// The discord guild id to delete
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
    },
}

/// Represents the user sub command, used to interact with the user storage
#[derive(Debug, Subcommand)]
#[clap()]
pub enum UserCmd {
    /// List all users with their wallet addresses in plain text, this spills
    /// sensitive data to the console, so be careful
    List {
        /// Starting index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "0")]
        start: u64,
        /// End index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "100")]
        end: u64,
    },
    /// Add a new user
    Add {
        /// The discord user id
        #[clap(value_hint = ValueHint::Other)]
        user_id: u64,
        /// The etherum wallet address
        #[clap(value_hint = ValueHint::Other)]
        wallet_address: String,
    },
    /// Remove a user
    Remove {
        /// The discord user id to delete
        #[clap(value_hint = ValueHint::Other)]
        user_id: u64,
    },
}

/// Represents the gates sub command, used to interact with the gates storage
#[derive(Debug, Subcommand)]
#[clap()]
pub enum GateCmd {
    /// List all gates
    List {
        /// The discord guild id
        #[clap(short, long)]
        guild: Option<u64>,
        /// Starting index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "0")]
        start: u64,
        /// End index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "100")]
        end: u64,
    },
    /// Remove a gate
    Remove {
        /// The guild id
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
        /// The gates identifier in the guild to delete
        #[clap(value_hint = ValueHint::Other)]
        identifier: u128,
    },
}

/// This structs contains the configuration for the application from command
/// line flags that take precedence over the config files and environment
/// variables. Most of the fields are optional and will be merged with other
/// sources values in the config module
#[derive(Args, Clone, Debug, Default, Deserialize)]
#[clap()]
pub struct CliConfig {
    /// Sets a custom config file
    #[clap(short, long,  value_name = "FILE", value_hint = ValueHint::FilePath, global(true))]
    pub config_file: Option<PathBuf>,
    /// The time it takes for a session to expire in seconds
    #[clap(long, short, global(true))]
    pub session_expiration: Option<u64>,
    #[clap(long, global(true))]
    pub internal_timeout: Option<u64>,
    /// Start the bot in maintenance mode, this will do nothing except telling
    /// discord users that the bot is in maintenance mode. This allows
    /// manipulating the storage in the meantime
    #[clap(long, short)]
    pub maintenance: Option<bool>,
    #[clap(flatten)]
    pub observability: CliObservabilityConfig,
    #[clap(flatten)]
    pub discord: CliDiscordConfig,
    #[clap(flatten)]
    pub server: CliServerConfig,
    #[clap(flatten)]
    pub storage: CliStorageConfig,
}

/// This structs contains the sub configuration for the logging and monitoring
/// options. Just for structuring the cli flags
#[derive(Args, Clone, Debug, Default, Deserialize)]
#[clap()]
pub struct CliObservabilityConfig {
    /// Define the verbosity of the application, repeat for more verbosity
    #[clap(long, short = 'v', global(true), parse(from_occurrences))]
    pub verbose: u8,
    #[clap(long, short, global(true), conflicts_with = "verbose")]
    /// Suppress all logging
    pub quiet: bool,
    #[cfg(feature = "jaeger-telemetry")]
    /// The jaeger endpoint to send the traces to
    #[clap(long, short, global(true))]
    pub jaeger_endpoint: Option<String>,
}

/// This structs contains the sub configuration for the discord client options.
/// Just for structuring the cli flags
#[derive(Args, Clone, Debug, Default, Deserialize)]
#[clap()]
pub struct CliDiscordConfig {
    /// The discord bot token
    #[clap(short, long, global(true))]
    pub token: Option<SecretString>,
    /// The discor bot invitation url
    #[clap(short, long, global(true))]
    pub invite_url: Option<String>,
}

/// This structs contains the sub configuration for the http server options.
/// Just for structuring the cli flags
#[derive(Args, Clone, Debug, Default, Deserialize)]
#[clap()]
pub struct CliServerConfig {
    /// The address to listen on
    #[clap(short = 'H', long, global(true))]
    pub host: Option<String>,
    /// The base url under which the server is reachable
    #[clap(short, long, global(true))]
    pub url: Option<String>,
    /// The port to listen on
    #[clap(short = 'P', long, global(true))]
    pub port: Option<u16>,
}

/// This structs contains the sub configuration for the storage options.
/// Just for structuring the cli flags
#[derive(Args, Clone, Debug, Default, Deserialize)]
#[clap()]
pub struct CliStorageConfig {
    /// The path where the persistent data is stored
    #[clap(short, long, global(true))]
    pub directory: Option<PathBuf>,
    /// How to store data, on disk or in memory
    #[clap(short = 'S', long, global(true))]
    pub storage_type: Option<StorageType>,
    /// The encryption_key used to encrypt the stored data
    #[clap(short, long, global(true))]
    pub key: Option<SecretString>,
}

/// The storage type enum, used to select the storage type
#[derive(Clone, Debug, Deserialize)]
pub enum StorageType {
    /// Store data peristent and encrypted on disk, this is the default
    Encrypted,
    /// Store data peristent but unencrypted on disk
    Unencrypted,
    /// Store data in memory, this is not persistent
    InMemory,
}

impl std::str::FromStr for StorageType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Encrypted" => Ok(StorageType::Encrypted),
            "Unencrypted" => Ok(StorageType::Unencrypted),
            "InMemory" => Ok(StorageType::InMemory),
            _ => Err(format!("Invalid storage type: {}", s)),
        }
    }
}
