//! Centralized logging for Panorama services.
//!
//! Call `panorama_logging::init("service-name", Some("/path/to/logs.db"))` at startup.
//! This installs a tracing subscriber that:
//! - Prints formatted logs to stderr (like the old `tracing_subscriber::fmt()`)
//! - Persists WARN+ events to a SQLite database via a background writer

mod layer;
mod writer;

use std::sync::OnceLock;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub use layer::DatastoreLayer;
pub use writer::{persist_error, LogRecord, LogSender, LogWriter};

/// Keeps the LogWriter alive for the process lifetime so the background
/// thread isn't dropped prematurely.
static LOG_WRITER: OnceLock<LogWriter> = OnceLock::new();

/// Initialize the global tracing subscriber for a Panorama service.
///
/// - `service`: the service name tag attached to every log record.
/// - `db_path`: optional path to the SQLite log database. If `None`, only
///   stderr output is produced (useful for tests).
pub fn init(service: &str, db_path: Option<&str>) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    if let Some(path) = db_path {
        let writer =
            LogWriter::open(path, service).expect("Failed to open log database");
        let sender = writer.sender();

        // Store writer in static so the background thread lives forever.
        let _ = LOG_WRITER.set(writer);

        let ds_layer = DatastoreLayer::new(sender, service.to_string());
        registry.with(ds_layer).init();
    } else {
        registry.init();
    }
}
