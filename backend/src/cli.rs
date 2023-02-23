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

/// The long description used for the help subcommand and man page.
/// It is sourced from the cargo readme template, removing all template
/// expression from the file.
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
    /// The main configuration params live here and can be set with command line
    /// flags
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
    /// Print the configuration and get a template file
    #[clap(subcommand)]
    Config(ConfigCmd),
    /// Interact with the presistent storage and encryption
    #[clap(subcommand)]
    Storage(StorageCmd),
    /// Interact with discord directly, e.g. register slash commands
    #[clap(subcommand)]
    Discord(DiscordCmd),
}

/// Represents the config sub command, used to print the current config or get a template
#[derive(Subcommand)]
#[clap()]
pub enum ConfigCmd {
    /// Print the configuration sources and merged config
    Show,
    /// Prints an example configuration template
    Template,
}

/// represents the discord sub command, used to register and delete slash commands
#[derive(Subcommand)]
#[clap()]
pub enum DiscordCmd {
    /// Register the global slash commands
    #[clap(subcommand)]
    Register(RegisterCmd),
    /// Register the slash commands for a specific guild
    #[clap(subcommand)]
    Delete(DeleteCmd),
}

/// represents the discord sub command, used to register slash commands in
/// a specific guild or globally
#[derive(Subcommand)]
#[clap()]
pub enum RegisterCmd {
    /// Register the global slash commands
    Global,
    /// Register the slash commands for a specific guild
    Guild {
        /// The guild id
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
    },
}

/// represents the discord sub command, used to delete slash commands in
/// a specific guild or globally
#[derive(Subcommand)]
#[clap()]
pub enum DeleteCmd {
    /// Register the global slash commands
    Global,
    /// Register the slash commands for a specific guild
    Guild {
        /// The guild id
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
    },
}

/// Represents the storage sub command, used to interact with the stored data
/// and encryption. Commands that use the data on disk, only work if the
/// bot is not running, otherwise the data is locked.
#[derive(Subcommand)]
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
#[derive(Subcommand)]
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
        /// The discord user id
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
    },
}

/// Represents the user sub command, used to interact with the user storage
#[derive(Subcommand)]
#[clap()]
pub enum UserCmd {
    /// List all users
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
        /// The discord user id
        #[clap(value_hint = ValueHint::Other)]
        user_id: u64,
    },
}

/// Represents the gates sub command, used to interact with the gates storage
#[derive(Subcommand)]
#[clap()]
pub enum GateCmd {
    /// List all gates
    List {
        /// The discord guild(server) id
        #[clap(short, long)]
        guild: Option<u64>,
        /// Starting index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "0")]
        start: u64,
        /// End index of the listed entries
        #[clap(value_hint = ValueHint::Other, default_value = "100")]
        end: u64,
        /// List gates in all guilds
        #[clap(short, long, conflicts_with = "guild")]
        all_guilds: bool,
    },
    /// Add a new gate
    Add {
        /// The guild id
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
        /// The colony address
        #[clap(value_hint = ValueHint::Other)]
        colony_address: String,
        /// The domain id
        #[clap(value_hint = ValueHint::Other)]
        domain_id: u64,
        /// The percentage of reputation needed to get the role
        #[clap(value_hint = ValueHint::Other)]
        reputation: u8,
        /// The discord role id
        #[clap(value_hint = ValueHint::Other)]
        role_id: u64,
    },
    /// Remove a gate
    Remove {
        /// The guild id
        #[clap(value_hint = ValueHint::Other)]
        guild_id: u64,
        /// The colony address
        #[clap(value_hint = ValueHint::Other)]
        colony_address: String,
        /// The domain
        #[clap(value_hint = ValueHint::Other)]
        domain_id: u64,
        /// The percentage of reputation needed to get the role
        #[clap(value_hint = ValueHint::Other)]
        reputation: u8,
        /// The discord role id
        #[clap(value_hint = ValueHint::Other)]
        role_id: u64,
    },
}

/// This structs contains the configuration for the application from command
/// line flags that take precedence over the config files and environment
/// variables. Most of the fields are optional and will be merged with other
/// sources values in the config module
#[derive(Args, Clone, Debug, Default, Serialize, Deserialize)]
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
    pub storage: CliStorageConfig,
}

/// This structs contains the sub configuration for the discord client options.
/// Just for structuring the cli flags
#[derive(Args, Clone, Debug, Default, Serialize, Deserialize)]
#[clap()]
pub struct CliDiscordConfig {
    /// The discord bot token
    #[clap(short, long)]
    pub token: Option<String>,
}

/// This structs contains the sub configuration for the http server options.
/// Just for structuring the cli flags
#[derive(Args, Clone, Debug, Default, Serialize, Deserialize)]
#[clap()]
pub struct CliServerConfig {
    /// The address to listen on
    #[clap(short, long)]
    pub host: Option<String>,
    /// The base url under which the server is reachable
    #[clap(short, long)]
    pub url: Option<String>,
    /// The port to listen on
    #[clap(short, long)]
    pub port: Option<u16>,
}

/// This structs contains the sub configuration for the storage options.
/// Just for structuring the cli flags
#[derive(Args, Clone, Debug, Default, Serialize, Deserialize)]
#[clap()]
pub struct CliStorageConfig {
    /// The path where the persistent data is stored
    #[clap(short, long)]
    pub directory: Option<PathBuf>,
    /// How to store data, on disk or in memory
    #[clap(short, long)]
    pub storage_type: Option<StorageType>,
    /// The encryption_key used to encrypt the stored data
    #[clap(short, long)]
    pub key: Option<String>,
}

/// The storage type enum, used to select the storage type
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StorageType {
    /// Store data peristent and encrypted on disk, this is the default
    Encrypted,
    /// Store data peristent but unencrypted on disk
    Unencrypted,
    /// Store data in memory, this is not persistent
    InMemory,
}

impl Default for StorageType {
    fn default() -> Self {
        Self::Encrypted
    }
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
