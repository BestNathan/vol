//! Tracing and logging initialization.
//!
//! Sets up:
//! - Console layer (compact format, colored)
//! - File layer (JSON format, rolling daily, 7-day retention)
//! - Error file layer (ERROR level only)

use std::sync::OnceLock;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use vol_config::{LoggingConfig, TracingConfig};

static TRACING_INITIALIZED: OnceLock<()> = OnceLock::new();

/// Initialize tracing and logging.
///
/// Call this once at application startup.
/// Subsequent calls are no-ops.
pub fn init(config: &TracingConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if already initialized (prevent double init in tests)
    if TRACING_INITIALIZED.get().is_some() {
        return Ok(());
    }

    // Create log directory
    std::fs::create_dir_all(&config.logging.log_dir)?;

    // Build EnvFilter for dynamic log levels
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.logging.console_level));

    // 1. Console layer (compact, colored)
    let console_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .with_ansi(true);

    // 2. File layer (JSON, rolling)
    let file_appender = create_file_appender(&config.logging);
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .with_writer(file_appender);

    // 3. Error file layer (ERROR only)
    let error_layer = config.logging.error_file.then(|| {
        let error_appender = create_error_appender(&config.logging);
        let error_filter = tracing_subscriber::filter::LevelFilter::ERROR;
        fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .json()
            .with_writer(error_appender)
            .with_filter(error_filter)
    });

    // Build subscriber - handle both cases
    match error_layer {
        None => {
            tracing_subscriber::Registry::default()
                .with(env_filter)
                .with(console_layer)
                .with(file_layer)
                .init();
        }
        Some(error) => {
            tracing_subscriber::Registry::default()
                .with(env_filter)
                .with(console_layer)
                .with(file_layer)
                .with(error)
                .init();
        }
    }

    // Mark as initialized
    TRACING_INITIALIZED.get_or_init(|| ());

    tracing::info!("Tracing initialized");

    Ok(())
}

fn create_file_appender(config: &LoggingConfig) -> RollingFileAppender {
    RollingFileAppender::new(
        Rotation::DAILY,
        &config.log_dir,
        format!("{}.log", config.log_prefix),
    )
}

fn create_error_appender(config: &LoggingConfig) -> RollingFileAppender {
    RollingFileAppender::new(
        Rotation::DAILY,
        &config.log_dir,
        format!("{}.error.log", config.log_prefix),
    )
}

pub fn shutdown() {
    tracing::info!("Shutting down OpenTelemetry");
}
