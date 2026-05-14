//! vol-observability: Independent observability service for ReAct Agent events.

use std::net::SocketAddr;

use vol_observability::config::ObservabilityConfig;
use vol_observability::ingest::{build_router, AppState};
use vol_observability::loki_writer::spawn_loki_writer;
use vol_observability::tdengine_writer::spawn_tdengine_writer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("vol_observability=info".parse().unwrap()),
        )
        .init();

    let config = load_config();

    tracing::info!(
        listen_addr = %config.listen_addr,
        loki_enabled = config.loki.enabled,
        tdengine_enabled = config.tdengine.enabled,
        "Starting vol-observability service",
    );

    // Parse TDengine DSN into HTTP REST URL
    let tdengine_base_url = parse_tdengine_dsn_to_http(&config.tdengine.dsn);

    // Spawn Loki writer
    let (loki_tx, loki_health) = spawn_loki_writer(
        config.loki.url.clone(),
        config.loki.batch_size,
        config.loki.flush_interval_ms,
    );

    // Spawn TDengine writer (default credentials: root/taosdata)
    let (tdengine_tx, tdengine_health) = spawn_tdengine_writer(
        tdengine_base_url,
        "root".to_string(),
        "taosdata".to_string(),
        config.tdengine.database.clone(),
        config.tdengine.batch_size,
        config.tdengine.flush_interval_ms,
    );

    let app_state = AppState {
        loki_tx,
        tdengine_tx,
        loki_health,
        tdengine_health,
    };

    let app = build_router(app_state);

    let addr: SocketAddr = config
        .listen_addr
        .parse()
        .expect("Invalid listen address");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener");

    tracing::info!(%addr, "Listening");

    axum::serve(listener, app)
        .await
        .unwrap();
}

fn load_config() -> ObservabilityConfig {
    // Try to load from TOML config file via env var
    if let Ok(config_path) = std::env::var("VOL_OBSERVABILITY_CONFIG") {
        if let Ok(config_str) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str::<ObservabilityConfig>(&config_str) {
                return config;
            }
        }
    }
    ObservabilityConfig::default()
}

/// Parse TDengine DSN (taos://host:port) into HTTP REST URL (http://host:6041).
/// TDengine native port (6030) differs from REST port (6041).
fn parse_tdengine_dsn_to_http(dsn: &str) -> String {
    let without_scheme = dsn.strip_prefix("taos://").unwrap_or(dsn);

    if let Some(colon_pos) = without_scheme.find(':') {
        let host = without_scheme[..colon_pos].to_string();
        let native_port: u16 = without_scheme[colon_pos + 1..]
            .parse()
            .unwrap_or(6030);
        // Convert native port (6030) to REST port (6041)
        let rest_port = if native_port == 6030 { 6041 } else { native_port + 11 };
        format!("http://{}:{}", host, rest_port)
    } else {
        // Default: localhost REST port
        "http://localhost:6041".to_string()
    }
}
