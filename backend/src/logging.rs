//! The logging module sets up the logging system as specified in the
//! configuration.
//!
//! It uses the [tracing](https://crates.io/crates/tracing)
//! crate to make it easy to trace the execution of the program through
//! different threads or asynchronous execution.
//!
//! The verbosity can be controlled via the verbosity config option.
use crate::config::CONFIG;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace, warn};
use tracing_appender::non_blocking::WorkerGuard;

/// The worker guard for the tracing appender to keep it from beeing dropped
static GUARD: OnceCell<WorkerGuard> = OnceCell::new();

/// The logging module sets up the logging system as specified in
/// configuration.
pub fn setup_logging() {
    if let Err(err) = tracing_log::LogTracer::init() {
        println!("Failed to initialize log tracer: {}", err);
    }
    // create appender for standard error
    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stderr());
    GUARD.set(guard).expect("Failed to set appender guard");

    let tracing_level = match CONFIG.wait().verbosity {
        LogLevel::Off => None,
        LogLevel::Error => Some(tracing::Level::ERROR),
        LogLevel::Warn => Some(tracing::Level::WARN),
        LogLevel::Info => Some(tracing::Level::INFO),
        LogLevel::Debug => Some(tracing::Level::DEBUG),
        LogLevel::Trace => Some(tracing::Level::TRACE),
    };
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_writer(non_blocking)
        .with_max_level(tracing_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting the default tracing subscriber failed");
    match CONFIG.wait().verbosity {
        LogLevel::Off => {}
        LogLevel::Error => {}
        LogLevel::Warn => warn!("Verbosity level set to warn"),
        LogLevel::Info => info!("Verbosity level set to info"),
        LogLevel::Debug => debug!("Verbosity level set to debug"),
        LogLevel::Trace => trace!("Verbosity level set to trace"),
    }
    trace!("Logging initialized");
}


/// The different log levels, from quiet = 0 to trace = 5
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Off = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Error
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
