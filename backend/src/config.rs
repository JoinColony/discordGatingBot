use crate::cli::CliConfig;
use crate::logging::LogLevel;
use confique::{toml, toml::FormatOptions, Config, File, FileFormat, Partial};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub static CONFIG: OnceCell<GlobalConfig> = OnceCell::new();
type PartialConf = <GlobalConfig as Config>::Partial;
type PartialServerConf = <ServerConfig as Config>::Partial;
type PartialAcmeConf = <AcmeConfig as Config>::Partial;
type PartialDiscordConf = <DiscordConfig as Config>::Partial;
type PartialEncryptionConf = <EncryptionConfig as Config>::Partial;

/// The configuration is loaded in the following order:
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
    let cfg = GlobalConfig::from_partial(merged).unwrap();
    CONFIG.set(cfg).expect("Failed to set config");
    Ok(())
}

pub fn print_config(raw_cli_cfg: &CliConfig) {
    let (cli_cfg, env, file, default, config_file) = get_config_hirarchy(&raw_cli_cfg);
    let default_config = GlobalConfig::from_partial(default).unwrap();
    println!("Default config: {:#?}", default_config);

    if !env.is_empty() {
        println!("\n\nEnvironment variables: ");
        std::env::vars()
            .filter(|(k, _)| k.starts_with("CLNY_"))
            .for_each(|(k, v)| println!("{}={}", k, v));
    }

    if let Ok(file_content) = fs::read_to_string(&config_file) {
        println!(
            "\n\nUsed config file: {}: \n{}",
            config_file.display(),
            file_content
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
    let cfg = GlobalConfig::from_partial(merged).unwrap();

    println!("\n\nMerged config: {:#?}", cfg);
}

fn get_config_hirarchy(
    raw_cli_cfg: &CliConfig,
) -> (PartialConf, PartialConf, PartialConf, PartialConf, PathBuf) {
    let cli_cfg = PartialConf {
        config_file: raw_cli_cfg.config_file.clone(),
        verbosity: match (raw_cli_cfg.verbose, raw_cli_cfg.quiet) {
            (_, true) => Some(LogLevel::Off),
            (0, _) => None,
            (1, _) => Some(LogLevel::Warn),
            (2, _) => Some(LogLevel::Info),
            (3, _) => Some(LogLevel::Debug),
            _ => Some(LogLevel::Trace),
        },
        discord: PartialDiscordConf {
            token: raw_cli_cfg.discord.token.clone(),
            shards: raw_cli_cfg.discord.shards.clone(),
        },
        server: PartialServerConf {
            host: raw_cli_cfg.server.host.clone(),
            port: raw_cli_cfg.server.port,
            cert: raw_cli_cfg.server.cert.clone(),
            key: raw_cli_cfg.server.key.clone(),
        },
        acme: PartialAcmeConf {
            acme_endpoint: raw_cli_cfg.acme.acme_endpoint.clone(),
            acme_port: raw_cli_cfg.acme.acme_port,
            staging: raw_cli_cfg.acme.staging,
        },
        encryption: PartialEncryptionConf {
            key: raw_cli_cfg.encryption.encryption_key.clone(),
        },
    };
    let env = PartialConf::from_env().unwrap();
    let config_file = if let Some(ref config_file) = cli_cfg.config_file {
        config_file.clone()
    } else if let Some(ref config_file) = env.config_file {
        config_file.clone()
    } else {
        PathBuf::from("config.toml")
    };
    let file: PartialConf = File::with_format(&config_file, FileFormat::Toml)
        .load()
        .unwrap();

    let default = PartialConf::default_values();
    (cli_cfg, env, file, default, config_file)
}

pub fn print_template() {
    println!(
        "{}",
        toml::template::<GlobalConfig>(FormatOptions::default())
    );
}

/// The main configuration struct, it contains all the configuration options
/// that can be set via the config file, environment variables or cli flags
/// and is used to configure the application
///
#[derive(Clone, Config, Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[config(env = "CLNY_CONFIG_FILE", default = "config.toml")]
    pub config_file: PathBuf,
    #[config(env = "CLNY_VERBOSITY", default = "Error")]
    pub verbosity: LogLevel,

    #[config(nested)]
    pub discord: DiscordConfig,
    #[config(nested)]
    pub server: ServerConfig,
    #[config(nested)]
    pub acme: AcmeConfig,
    #[config(nested)]
    pub encryption: EncryptionConfig,
}

#[derive(Clone, Config, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    /// The address to listen on
    #[config(env = "CLNY_HOST", default = "localhost")]
    pub host: String,
    /// The port to listen on
    #[config(env = "CLNY_PORT", default = 8080)]
    pub port: u16,
    /// The path to the certificate File
    #[config(env = "CLNY_CERT", default = "./cert.pem")]
    pub cert: PathBuf,
    /// The path to the private key File
    #[config(env = "CLNY_KEY", default = "./key.pem")]
    pub key: PathBuf,
}

#[derive(Clone, Config, Debug, Serialize, Deserialize)]
pub struct AcmeConfig {
    /// The address of the acme server to use
    #[config(env = "CLNY_ACME_ENDPOINT", default = "acme-v02.api.letsencrypt.org")]
    pub acme_endpoint: String,
    /// The port to listen on
    #[config(env = "CLNY_ACME_PORT", default = 8081)]
    pub acme_port: u16,
    /// The path to the directory where the certificates are stored
    #[config(env = "CLNY_STAGING", default = true)]
    pub staging: bool,
}

#[derive(Clone, Config, Debug, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// The discord bot token
    #[config(env = "CLNY_DISCORD_TOKEN", default = "")]
    pub token: String,
    #[config(env = "CLNY_DISCORD_SHARDS", default = 1)]
    pub shards: u8,
}

#[derive(Clone, Config, Debug, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// The discord bot token
    #[config(env = "CLNY_ENCRYPTION_KEY", default = "")]
    pub key: String,
}
