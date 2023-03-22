//! The global configuration is loaded and set up here as a global static
//! OnceCell
//!

use crate::cli::{CliConfig, StorageType};
use crate::logging::LogLevel;
use confique::{toml, toml::FormatOptions, Config, File, FileFormat, Partial};
use once_cell::sync::OnceCell;
use regex::Regex;
use secrecy::SecretString;
use serde::Deserialize;
use std::{fs, path::PathBuf};

/// The global configuration is loaded into a global static OnceCell
/// and can be accessed from there by all parts of the application
pub static CONFIG: OnceCell<GlobalConfig> = OnceCell::new();

/// Partial configuration used to construct the final configuration
type PartialConf = <GlobalConfig as Config>::Partial;
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
    let default_config =
        GlobalConfig::from_partial(default).expect("Invalid default configuration");
    println!("Default config: {:#?}", default_config);

    if !env.is_empty() {
        println!("\n\nEnvironment variables: ");
        std::env::vars()
            .filter_map(|(k, v)| {
                if k.starts_with("CLNY_") {
                    if k == "CLNY_ENCRYPTION_KEY" {
                        return Some((k, "<redacted>".to_string()));
                    }
                    if k == "CLNY_DISCORD_TOKEN" {
                        return Some((k, "<redacted>".to_string()));
                    }
                    Some((k, v))
                } else {
                    None
                }
            })
            .for_each(|(k, v)| println!("{}={}", k, v));
    }

    if let Ok(file_content) = fs::read_to_string(&config_file) {
        let token_re = Regex::new(r"(?m)^\s*token\s*=.*")
            .expect("Invalid hardcoded regex, this should not happen");
        let key_re = Regex::new(r"(?m)^\s*key\s*=.*")
            .expect("Invalid hardcoded regex, this should not happen");
        let without_token = token_re.replace_all(&file_content, "token = \"<redacted>\"");
        let without_key = key_re.replace_all(&without_token, "key = \"<redacted>\"");
        println!(
            "\n\nUsed config file: {}: \n{}",
            config_file.display(),
            without_key
        );
    }

    if !cli_cfg.is_empty() {
        println!("\n\nCLI flags: {:#?}", raw_cli_cfg);
    }

    let default = PartialConf::default_values();
    let merged = cli_cfg
        .with_fallback(env)
        .with_fallback(file)
        .with_fallback(default);
    let cfg = GlobalConfig::from_partial(merged).expect("Invalid configuration");

    println!("\n\nMerged config: {:#?}", cfg);
}

/// Gets all partial configurations from the different sources.
/// It also does the special handling of verbose and quiet flags
fn get_config_hirarchy(
    raw_cli_cfg: &CliConfig,
) -> (PartialConf, PartialConf, PartialConf, PartialConf, PathBuf) {
    let cli_cfg = PartialConf {
        config_file: raw_cli_cfg.config_file.clone(),
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
        session_expiration: raw_cli_cfg.session_expiration,
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

/// The main configuration struct used by the entire application
/// it is constructed from the partial configurations from different sources
#[derive(Clone, Config, Debug, Deserialize)]
pub struct GlobalConfig {
    /// The path to the configuration file
    #[config(env = "CLNY_CONFIG_FILE", default = "config.toml")]
    pub config_file: PathBuf,
    /// The time it takes for a session to expire in seconds
    #[config(env = "CLNY_SESSION_EXPIRATION", default = 60)]
    pub session_expiration: u64,
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
    #[config(env = "CLNY_VERBOSITY", default = "Error")]
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
    #[config(env = "CLNY_URL", default = "http://localhost")]
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
    #[config(env = "CLNY_STORAGE_TYPE", default = "Encrypted")]
    pub storage_type: StorageType,
    /// The encryption_key used to encrypt the stored data
    #[config(env = "CLNY_ENCRYPTION_KEY", default = "")]
    pub key: SecretString,
}

/// The sub configuration for discord interaction
#[derive(Clone, Config, Debug, Deserialize)]
pub struct DiscordConfig {
    /// The discord bot token
    #[config(env = "CLNY_DISCORD_TOKEN", default = "")]
    pub token: SecretString,
    /// The discor bot invitation url
    #[config(env = "CLNY_DISCORD_INVITATION_URL", default = "")]
    pub invite_url: String,
}
