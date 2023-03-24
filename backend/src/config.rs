//! The global configuration is loaded and set up here as a global static
//! OnceCell
//!

use crate::cli::{CliConfig, StorageType};
use crate::logging::LogLevel;
use confique::{toml, toml::FormatOptions, Config, File, FileFormat, Partial};
use once_cell::sync::OnceCell;
use secrecy::SecretString;
use serde::Deserialize;
use std::{path::PathBuf, str::FromStr};

/// The global configuration is loaded into a global static OnceCell
/// and can be accessed from there by all parts of the application
pub static CONFIG: OnceCell<GlobalConfig> = OnceCell::new();

/// The main configuration struct used by the entire application
/// it is constructed from the partial configurations from different sources
#[derive(Clone, Config, Debug, Deserialize)]
pub struct GlobalConfig {
    /// The path to the configuration file, specifiying this in the config
    /// file itself does not have any effect since the config file is loaded
    /// already
    #[config(env = "CLNY_CONFIG_FILE", default = "config.toml")]
    pub config_file: PathBuf,
    /// The time it takes for a session to expire in seconds
    #[config(env = "CLNY_SESSION_EXPIRATION", default = 60)]
    pub session_expiration: u64,
    /// Timout used for internal requests in milliseconds
    #[config(env = "CLNY_INTERNAL_TIMEOUT", default = 2000)]
    pub internal_timeout: u64,
    /// Start the bot in maintenance mode, this will do nothing except telling
    /// discord users that the bot is in maintenance mode
    #[config(env = "CLNY_MAINTENANCE", default = false)]
    pub maintenance: bool,
    #[config(nested)]
    pub observability: ObservabilityConfig,
    /// The discord configuration
    #[config(nested)]
    pub discord: DiscordConfig,
    /// The configuration of the https server used for the registration
    #[config(nested)]
    pub server: ServerConfig,
    /// The configuration of the storage backend and encryption
    #[config(nested)]
    pub storage: StorageConfig,
}

#[derive(Clone, Config, Debug, Deserialize)]
pub struct ObservabilityConfig {
    /// The log level, can be one of: Off, Error, Warn, Info, Debug, Trace
    #[config(env = "CLNY_VERBOSITY", parse_env = parse_from_env::<LogLevel>, default = "Error")]
    pub verbosity: LogLevel,
    #[cfg(feature = "jaeger-telemetry")]
    /// The jaeger endpoint to send the traces to
    #[config(env = "CLNY_JAEGER_ENDPOINT", default = "127.0.0.1:6831")]
    pub jaeger_endpoint: String,
}

/// The sub configuration for the http server
#[derive(Clone, Config, Debug, Deserialize)]
pub struct ServerConfig {
    /// The base url under which the server is reachable
    #[config(env = "CLNY_URL", default = "http://localhost:8080")]
    pub url: String,
    /// The address to listen on
    #[config(env = "CLNY_HOST", default = "localhost")]
    pub host: String,
    /// The port to listen on
    #[config(env = "CLNY_PORT", default = 8080)]
    pub port: u16,
}

/// The sub configuration for storage and encryption
#[derive(Clone, Config, Debug, Deserialize)]
pub struct StorageConfig {
    /// The path where the persistent data is stored
    #[config(env = "CLNY_STORAGE_DIRECTORY", default = "./data")]
    pub directory: PathBuf,
    /// How to store data, on disk or in memory
    #[config(env = "CLNY_STORAGE_TYPE",parse_env = parse_from_env::<StorageType>,  default = "Encrypted")]
    pub storage_type: StorageType,
    /// The encryption_key used to encrypt the stored data
    #[config(env = "CLNY_ENCRYPTION_KEY")]
    pub key: SecretString,
}

/// The sub configuration for discord interaction
#[derive(Clone, Config, Debug, Deserialize)]
pub struct DiscordConfig {
    /// The discord bot token
    #[config(env = "CLNY_DISCORD_TOKEN")]
    pub token: SecretString,
    /// The discor bot invitation url
    #[config(env = "CLNY_DISCORD_INVITATION_URL")]
    pub invite_url: String,
}

/// Partial configuration used to construct the final configuration
type PartialConf = <GlobalConfig as Config>::Partial;

/// Partial sub configuration of the observability sub configuration
/// used as part of the enclosing partial configuration
type PartialObservabilityConf = <ObservabilityConfig as Config>::Partial;
/// Partial sub configuration of the server sub configuration
/// used as part of the enclosing partial configuration
type PartialServerConf = <ServerConfig as Config>::Partial;
/// Partial sub configuration of the storage sub configuration
/// used as part of the enclosing partial configuration
type PartialStorageConf = <StorageConfig as Config>::Partial;
/// Partial sub configuration of the discord sub configuration
/// used as part of the enclosing partial configuration
type PartialDiscordConf = <DiscordConfig as Config>::Partial;

struct PrintablePartialConf {
    global: PartialConf,
    observability: PrintablePartialObservabilityConf,
    server: PrintablePartialServerConf,
    storage: PrintablePartialStorageConf,
    discord: PrintablePartialDiscordConf,
}

impl From<PartialConf> for PrintablePartialConf {
    fn from(mut global: PartialConf) -> Self {
        let observability = PrintablePartialObservabilityConf(std::mem::replace(
            &mut global.observability,
            PartialObservabilityConf::default_values(),
        ));
        let server = PrintablePartialServerConf(std::mem::replace(
            &mut global.server,
            PartialServerConf::default_values(),
        ));
        let storage = PrintablePartialStorageConf(std::mem::replace(
            &mut global.storage,
            PartialStorageConf::default_values(),
        ));
        let discord = PrintablePartialDiscordConf(std::mem::replace(
            &mut global.discord,
            PartialDiscordConf::default_values(),
        ));
        Self {
            global,
            observability,
            server,
            storage,
            discord,
        }
    }
}

impl std::fmt::Debug for PrintablePartialConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        s.push_str("\n");
        s.push_str(&format!("{}: {:?}", "config_file", self.global.config_file));
        s.push_str("\n");
        s.push_str(&format!(
            "{}: {:?}",
            "session_expiration", self.global.session_expiration
        ));
        s.push_str(&format!(
            "{}: {:?}",
            "internal_timeout", self.global.internal_timeout
        ));
        s.push_str("\n");
        s.push_str(&format!("{}: {:?}", "maintenance", self.global.maintenance));
        s.push_str("\n");
        s.push_str(&format!("{}: {:?}", "observability", &self.observability));
        s.push_str("\n");
        s.push_str(&format!("{}: {:?}", "discord", &self.discord));
        s.push_str("\n");
        s.push_str(&format!("{}: {:?}", "server", &self.server));
        s.push_str("\n");
        s.push_str(&format!("{}: {:?}", "storage", &self.storage));
        write!(f, "{}", s)
    }
}
struct PrintablePartialObservabilityConf(PartialObservabilityConf);
impl std::fmt::Debug for PrintablePartialObservabilityConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        s.push_str("\n");
        s.push_str(&format!(" {}: {:?}", "verbosity", self.0.verbosity));
        #[cfg(feature = "jaeger-telemetry")]
        s.push_str("\n");
        #[cfg(feature = "jaeger-telemetry")]
        s.push_str(&format!(
            " {}: {:?}",
            "jaeger_endpoint", self.0.jaeger_endpoint
        ));

        write!(f, "{}", s)
    }
}

struct PrintablePartialServerConf(PartialServerConf);
impl std::fmt::Debug for PrintablePartialServerConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        s.push_str("\n");
        s.push_str(&format!(" {}: {:?}", "url", self.0.url));
        s.push_str("\n");
        s.push_str(&format!(" {}: {:?}", "host", self.0.host));
        s.push_str("\n");
        s.push_str(&format!(" {}: {:?}", "port", self.0.port));

        write!(f, "{}", s)
    }
}

struct PrintablePartialStorageConf(PartialStorageConf);
impl std::fmt::Debug for PrintablePartialStorageConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        s.push_str(&format!("\n {}: {:?}\n", "directory", self.0.directory));
        s.push_str(&format!(" {}: {:?}\n", "storage_type", self.0.storage_type));
        s.push_str(&format!(" {}: {:?}\n", "key", self.0.key));

        write!(f, "{}", s)
    }
}

struct PrintablePartialDiscordConf(PartialDiscordConf);
impl std::fmt::Debug for PrintablePartialDiscordConf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        s.push_str(&format!("\n {}: {:?}", "token", self.0.token));
        write!(f, "{}", s)
    }
}

/// Merges all configuration sources and initializes the global configuration
///
/// The sources are loaded in the following order, later sources overwrite
/// earlier ones:
/// 1. Default values
/// 2. Config file
/// 3. Environment variables
/// 4. CLI flags
pub fn setup_config(raw_cli_cfg: &CliConfig) -> Result<(), String> {
    let (cli_cfg, env, file, default, _) = get_config_hirarchy(&raw_cli_cfg);
    let merged = cli_cfg
        .with_fallback(env)
        .with_fallback(file)
        .with_fallback(default);
    let cfg = GlobalConfig::from_partial(merged).expect("Invalid configuration");
    CONFIG.set(cfg).expect("Failed to set config");
    Ok(())
}

/// Prints the different sources and finally merged configuration to stdout
pub fn print_config(raw_cli_cfg: &CliConfig) {
    let (cli_cfg, env, file, default, config_file) = get_config_hirarchy(&raw_cli_cfg);
    println!(
        "\nThis is how the configuration was loaded from it's parts, \nif a partial \
        config is completely empty, \nit will be omitted form the output"
    );
    println!(
        "\n\nDefault partial config: {:#?}",
        PrintablePartialConf::from(default)
    );
    if !file.is_empty() {
        println!(
            "\n\nFile partial config from {:?}: {:#?}",
            config_file,
            PrintablePartialConf::from(file)
        );
    }
    if !env.is_empty() {
        println!(
            "\n\nEnvironment parital config: {:#?}",
            PrintablePartialConf::from(env)
        );
    }
    if !cli_cfg.is_empty() {
        println!(
            "\n\nCLI partial config: {:#?}",
            PrintablePartialConf::from(cli_cfg)
        );
    }

    let (cli_cfg, env, file, default, _) = get_config_hirarchy(&raw_cli_cfg);
    let merged = cli_cfg
        .with_fallback(env)
        .with_fallback(file)
        .with_fallback(default);

    let cfg = GlobalConfig::from_partial(merged).expect("Invalid configuration");

    println!("\n\nMerged final config: {:#?}", cfg);
}

/// Gets all partial configurations from the different sources.
/// It also does the special handling of verbose and quiet flags
fn get_config_hirarchy(
    raw_cli_cfg: &CliConfig,
) -> (PartialConf, PartialConf, PartialConf, PartialConf, PathBuf) {
    let cli_cfg = PartialConf {
        config_file: raw_cli_cfg.config_file.clone(),
        maintenance: raw_cli_cfg.maintenance,
        session_expiration: raw_cli_cfg.session_expiration,
        internal_timeout: raw_cli_cfg.internal_timeout,
        observability: PartialObservabilityConf {
            verbosity: match (
                raw_cli_cfg.observability.verbose,
                raw_cli_cfg.observability.quiet,
            ) {
                (_, true) => Some(LogLevel::Off),
                (0, _) => None,
                (1, _) => Some(LogLevel::Warn),
                (2, _) => Some(LogLevel::Info),
                (3, _) => Some(LogLevel::Debug),
                _ => Some(LogLevel::Trace),
            },
            #[cfg(feature = "jaeger-telemetry")]
            jaeger_endpoint: raw_cli_cfg.observability.jaeger_endpoint.clone(),
        },
        discord: PartialDiscordConf {
            token: raw_cli_cfg.discord.token.clone(),
            invite_url: raw_cli_cfg.discord.invite_url.clone(),
        },
        server: PartialServerConf {
            url: raw_cli_cfg.server.url.clone(),
            host: raw_cli_cfg.server.host.clone(),
            port: raw_cli_cfg.server.port,
        },
        storage: PartialStorageConf {
            directory: raw_cli_cfg.storage.directory.clone(),
            storage_type: raw_cli_cfg.storage.storage_type.clone(),
            key: raw_cli_cfg.storage.key.clone(),
        },
    };
    let env = PartialConf::from_env().expect("Could not build config from env");
    let config_file = if let Some(ref config_file) = cli_cfg.config_file {
        config_file.clone()
    } else if let Some(ref config_file) = env.config_file {
        config_file.clone()
    } else {
        PathBuf::from("config.toml")
    };
    let file: PartialConf = File::with_format(&config_file, FileFormat::Toml)
        .load()
        .expect("Could not build config from file");

    let default = PartialConf::default_values();
    (cli_cfg, env, file, default, config_file)
}

/// Prints a configuration file template to stdout, that can be used as a
/// starting point for a custom configuration file
pub fn print_template() {
    println!(
        "{}",
        toml::template::<GlobalConfig>(FormatOptions::default())
    );
}

fn parse_from_env<T: FromStr<Err = String>>(s: &str) -> Result<T, ConfigFromEnvError> {
    Ok(T::from_str(s)?)
}

#[derive(Debug)]
struct ConfigFromEnvError(String);

impl FromStr for ConfigFromEnvError {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ConfigFromEnvError(s.to_string()))
    }
}

impl From<String> for ConfigFromEnvError {
    fn from(s: String) -> Self {
        ConfigFromEnvError(s)
    }
}

impl std::fmt::Display for ConfigFromEnvError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Error while reading config from environment {}", self.0)
    }
}

impl std::error::Error for ConfigFromEnvError {
    fn description(&self) -> &str {
        "Error while reading config from environment"
    }
}

impl Default for StorageType {
    fn default() -> Self {
        Self::Encrypted
    }
}
