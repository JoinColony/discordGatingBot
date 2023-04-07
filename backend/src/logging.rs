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
use tracing::{debug, info, metadata::LevelFilter, trace, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{filter::Targets, prelude::*};

/// The worker guard for the tracing appender to keep it from beeing dropped
static GUARD: OnceCell<WorkerGuard> = OnceCell::new();

/// The logging module sets up the logging system as specified in
/// configuration.
pub fn setup_logging() {
    let tracing_level = CONFIG.wait().observability.verbosity.clone();

    // setting up the log tracer that forwards log messages to tracing
    if let Err(err) = tracing_log::LogTracer::init_with_filter(tracing_level.clone().into()) {
        println!("Failed to initialize log tracer: {}", err);
    }

    // create appender for standard error
    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stderr());
    GUARD.set(guard).expect("Failed to set appender guard");
    let targets_filter = match tracing_level {
        LogLevel::Trace => Targets::new().with_default(tracing_level.clone()),
        // For debug and above, we want to see only messages from this crate
        _ => Targets::new()
            .with_target(env!("CARGO_CRATE_NAME"), tracing_level.clone())
            .with_target("tracing_actix_web", tracing_level.clone()),
    };

    // configure the subscriber
    let subscriber = match tracing_level {
        LogLevel::Trace => tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NEW)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ENTER)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::EXIT)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .with_file(true)
            .with_line_number(true),
        _ => tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_file(true)
            .with_line_number(true),
    };

    #[cfg(feature = "jaeger-telemetry")]
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_endpoint(CONFIG.wait().observability.jaeger_endpoint.clone())
        .with_service_name("discord-gating-bot")
        .install_simple()
        .unwrap();

    #[cfg(feature = "jaeger-telemetry")]
    let telemetry = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(targets_filter.clone());

    #[cfg(feature = "jaeger-telemetry")]
    let registry = tracing_subscriber::registry()
        .with(targets_filter)
        .with(telemetry)
        .with(subscriber);

    #[cfg(not(feature = "jaeger-telemetry"))]
    let registry = tracing_subscriber::registry()
        .with(targets_filter)
        .with(subscriber);

    tracing::subscriber::set_global_default(registry)
        .expect("Setting the default tracing subscriber failed");

    tracing_level.print();
}

/// The different log levels, from quiet = 0 to trace = 5
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Off = 0,
    #[default]
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl LogLevel {
    fn print(&self) {
        match self {
            Self::Trace => {
                trace!("Verbosity level set to trace");
                #[cfg(feature = "jaeger-telemetry")]
                trace!(
                    "Jaeger telemetry enabled with endpoint: {}",
                    CONFIG.wait().observability.jaeger_endpoint
                );
            }
            Self::Debug => {
                debug!("Verbosity level set to debug");
                #[cfg(feature = "jaeger-telemetry")]
                debug!(
                    "Jaeger telemetry enabled with endpoint: {}",
                    CONFIG.wait().observability.jaeger_endpoint
                );
            }
            Self::Info => {
                info!("Verbosity level set to info");
                #[cfg(feature = "jaeger-telemetry")]
                info!(
                    "Jaeger telemetry enabled with endpoint: {}",
                    CONFIG.wait().observability.jaeger_endpoint
                );
            }
            Self::Warn => {
                warn!("Verbosity level set to warn");
                #[cfg(feature = "jaeger-telemetry")]
                warn!(
                    "Jaeger telemetry enabled with endpoint: {}, however many \
                    traces will only be enabled for higher verbosity",
                    CONFIG.wait().observability.jaeger_endpoint
                );
            }
            Self::Error => {
                #[cfg(feature = "jaeger-telemetry")]
                println!(
                    "Jaeger telemetry enabled with endpoint: {}, however many \
                    traces will only be enabled for higher verbosity",
                    CONFIG.wait().observability.jaeger_endpoint
                );
            }
            Self::Off => {}
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Off" => Ok(LogLevel::Off),
            "Error" => Ok(LogLevel::Error),
            "Warn" => Ok(LogLevel::Warn),
            "Info" => Ok(LogLevel::Info),
            "Debug" => Ok(LogLevel::Debug),
            "Trace" => Ok(LogLevel::Trace),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Off => LevelFilter::OFF,
            LogLevel::Error => LevelFilter::ERROR,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Trace => LevelFilter::TRACE,
        }
    }
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Off => log::LevelFilter::Off,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}
