//! vol-monitor: Main binary using channel-based engine.

mod state;
mod registry;

use anyhow::Result;
use tracing::{info, warn};
use tracing_subscriber::{self, EnvFilter};

use vol_config::Config;
use vol_engine::{MonitoringEngineBuilder, EngineConfig};
use vol_datasource::DeribitDataSource;
use vol_notification::{StdoutNotification, FeishuNotification};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("vol_monitor=info".parse().unwrap()))
        .init();

    info!("===========================================");
    info!("  Deribit Volatility Monitor v0.2.0");
    info!("===========================================");

    // Load configuration
    let config = Config::load("config.toml").unwrap_or_else(|e| {
        warn!("Failed to load config.toml: {}", e);
        create_default_config()
    });

    // Create datasource
    let deribit_config = config.data_sources.deribit.as_ref().expect("Deribit config required");
    let mut deribit_ds = DeribitDataSource::new(
        deribit_config.ws_url.clone(),
        deribit_config.symbols.clone(),
        deribit_config.poll_interval_secs,
    );

    // Use proxy if configured
    if let Ok(proxy) = std::env::var("HTTPS_PROXY").or_else(|_| std::env::var("HTTP_PROXY")) {
        info!("Using proxy: {}", proxy);
        deribit_ds = deribit_ds.with_proxy(proxy);
    }

    // Create notifications
    let stdout = StdoutNotification::new();
    let feishu = config.notifications.feishu.clone().map(FeishuNotification::new);

    // Build engine
    // Note: Rules will be added in a future task when vol-rules is migrated
    let mut builder = MonitoringEngineBuilder::new()
        .with_config(EngineConfig::default())
        .with_datasource(Box::new(deribit_ds))
        .with_notification(Box::new(stdout));

    if let Some(feishu_notif) = feishu {
        builder = builder.with_notification(Box::new(feishu_notif));
    }

    let engine = builder.build();

    info!("===========================================");
    info!("  Monitoring started");
    info!("===========================================");

    // Run engine (runs until shutdown)
    // TODO: Handle shutdown signal and save state
    let _ = engine.run().await;

    Ok(())
}

fn create_default_config() -> Config {
    use vol_config::*;
    use std::collections::HashMap;

    let mut symbols = HashMap::new();

    // BTC config - BTC typically has lower IV than ETH
    symbols.insert("btc".to_string(), SymbolIvConfig {
        short_threshold: 0.80,
        medium_threshold: 0.70,
        long_threshold: 0.60,
        short_atm_threshold: 0.05,
        medium_atm_threshold: 0.10,
        long_atm_threshold: 0.15,
    });

    // ETH config - ETH typically has higher IV, allow wider ATM ranges
    symbols.insert("eth".to_string(), SymbolIvConfig {
        short_threshold: 0.90,
        medium_threshold: 0.80,
        long_threshold: 0.70,
        short_atm_threshold: 0.08,
        medium_atm_threshold: 0.12,
        long_atm_threshold: 0.18,
    });

    Config {
        data_sources: DataSourcesConfig {
            enabled: vec!["deribit".to_string()],
            deribit: Some(DeribitConfig {
                ws_url: "wss://www.deribit.com/ws/api/v2".to_string(),
                symbols: vec!["BTC".to_string(), "ETH".to_string()],
                poll_interval_secs: 60,
                auth: None,
            }),
        },
        tenors: TenorConfig {
            short_max_dte: 7,
            medium_min_dte: 20,
            medium_max_dte: 40,
            long_min_dte: 80,
        },
        alerts: AlertsConfig {
            enabled: vec!["absolute_iv".to_string(), "rate_change".to_string()],
            cooldown_secs: 300,
            absolute_iv: AbsoluteIvConfig { symbols },
            rate_of_change: RateOfChangeConfig {
                window_1h_threshold: 0.05,
                window_4h_threshold: 0.10,
                window_24h_threshold: 0.20,
            },
            term_structure: TermStructureConfig {
                short_long_spread_threshold: 0.15,
            },
            skew: SkewConfig {
                threshold: 0.10,
            },
            metrics: vec![],
        },
        notifications: NotificationsConfig {
            enabled: vec!["stdout".to_string()],
            feishu: None,
        },
        state: StateConfig {
            path: "~/.deribit-vol-monitor/state.json".to_string(),
        },
    }
}
