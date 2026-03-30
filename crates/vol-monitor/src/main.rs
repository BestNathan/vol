//! vol-monitor: Main binary for the volatility monitoring service.

mod state;
mod registry;

use anyhow::Result;
use tracing::{info, warn, error};
use tracing_subscriber::{self, EnvFilter};

use vol_core::{DataSource, AlertHandler, NotificationHandler, VolatilityData};
use vol_config::Config;
use vol_datasource::DeribitDataSource;
use vol_alert::{AlertManager, AbsoluteIvHandler, RateChangeHandler, TermStructureHandler, SkewHandler};
use vol_notification::{StdoutNotification, FeishuNotification};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("vol_monitor=info".parse().unwrap()))
        .init();

    info!("===========================================");
    info!("  Deribit Volatility Monitor v0.1.0");
    info!("===========================================");

    // Load configuration
    let config = Config::load("config.toml").unwrap_or_else(|e| {
        warn!("Failed to load config.toml: {}", e);
        warn!("Using default configuration");
        create_default_config()
    });

    // Create alert manager
    let alert_manager = AlertManager::new(config.alerts.cooldown_secs);

    // Load previous state
    if let Ok(state) = state::load_state(&config.state.path) {
        alert_manager.load_state(state);
        info!("Loaded alert state from {}", config.state.path);
    }

    // Initialize data source
    let deribit_config = config.data_sources.deribit.as_ref().expect("Deribit config required");
    let mut deribit = DeribitDataSource::new(
        deribit_config.ws_url.clone(),
        deribit_config.symbols.clone(),
        deribit_config.poll_interval_secs,
    );

    // Use proxy if configured or via environment
    let proxy_url = std::env::var("HTTPS_PROXY").or_else(|_| std::env::var("HTTP_PROXY")).ok();
    if let Some(ref proxy) = proxy_url {
        info!("Using proxy from environment: {}", proxy);
        deribit = deribit.with_proxy(proxy.clone());
    }

    deribit.connect().await?;
    info!("Connecting to Deribit WebSocket: {}", deribit_config.ws_url);

    // Subscribe to data stream
    let mut data_rx = deribit.subscribe(deribit_config.symbols.clone())?;
    info!("Subscribed to ticker data for symbols: {:?}", deribit_config.symbols);

    // Initialize alert handlers
    let abs_iv_config = config.alerts.absolute_iv.clone();
    let absolute_iv = AbsoluteIvHandler::new(abs_iv_config.clone());
    let rate_change = RateChangeHandler::new(config.alerts.rate_of_change.clone());
    let term_structure = TermStructureHandler::new(config.alerts.term_structure.clone());
    let skew = SkewHandler::new(config.alerts.skew.clone());

    // Initialize notification handlers
    let stdout = StdoutNotification::new();
    let feishu = config.notifications.feishu.clone().map(FeishuNotification::new);

    info!("===========================================");
    info!("  Monitoring started - waiting for data...");
    info!("===========================================");
    info!("");
    info!("Alert thresholds (BTC):");
    info!("  Absolute IV:  short>={:.0}%, medium>={:.0}%, long>={:.0}%",
          abs_iv_config.get_symbol_config("btc").map(|c| c.short_threshold * 100.0).unwrap_or(0.0),
          abs_iv_config.get_symbol_config("btc").map(|c| c.medium_threshold * 100.0).unwrap_or(0.0),
          abs_iv_config.get_symbol_config("btc").map(|c| c.long_threshold * 100.0).unwrap_or(0.0));
    info!("Alert thresholds (ETH):");
    info!("  Absolute IV:  short>={:.0}%, medium>={:.0}%, long>={:.0}%",
          abs_iv_config.get_symbol_config("eth").map(|c| c.short_threshold * 100.0).unwrap_or(0.0),
          abs_iv_config.get_symbol_config("eth").map(|c| c.medium_threshold * 100.0).unwrap_or(0.0),
          abs_iv_config.get_symbol_config("eth").map(|c| c.long_threshold * 100.0).unwrap_or(0.0));
    info!("");

    // Main event loop
    let mut sample_count: u32 = 0;

    loop {
        tokio::select! {
            // Handle incoming volatility data
            Some(vol_data) = data_rx.recv() => {
                sample_count += 1;

                // Log received data
                info!("[{}] {} {} | IV: {:.1}% | DTE: {} days | Strike: ${:.0} | Index: ${:.0} | ITM: {}",
                    sample_count,
                    vol_data.symbol,
                    match vol_data.option_type {
                        vol_core::OptionType::Call => "C",
                        vol_core::OptionType::Put => "P",
                    },
                    vol_data.iv * 100.0,
                    vol_data.dte,
                    vol_data.strike,
                    vol_data.index_price,
                    vol_data.is_itm()
                );

                // Run alert evaluation
                evaluate_and_notify(&vol_data, &absolute_iv, &rate_change, &term_structure, &skew, &stdout, feishu.as_ref(), &alert_manager).await;
            }

            // Handle shutdown signal
            _ = tokio::signal::ctrl_c() => {
                info!("");
                info!("Shutting down vol-monitor...");
                break;
            }
        }
    }

    // Save state before exit
    state::save_state(&config.state.path, &alert_manager.get_state())?;
    info!("Saved alert state to {}", config.state.path);
    info!("Goodbye!");

    Ok(())
}

/// Evaluate volatility data against all alert handlers and send notifications
async fn evaluate_and_notify(
    vol_data: &VolatilityData,
    absolute_iv: &AbsoluteIvHandler,
    rate_change: &RateChangeHandler,
    term_structure: &TermStructureHandler,
    skew: &SkewHandler,
    stdout: &StdoutNotification,
    feishu: Option<&FeishuNotification>,
    alert_manager: &AlertManager,
) {
    let mut alerts_triggered = Vec::new();

    // Evaluate each alert handler
    if let Some(alert) = absolute_iv.evaluate(vol_data) {
        alerts_triggered.push((alert, "absolute_iv"));
    }

    if let Some(alert) = rate_change.evaluate(vol_data) {
        alerts_triggered.push((alert, "rate_change"));
    }

    if let Some(alert) = term_structure.evaluate(vol_data) {
        alerts_triggered.push((alert, "term_structure"));
    }

    if let Some(alert) = skew.evaluate(vol_data) {
        alerts_triggered.push((alert, "skew"));
    }

    // Process triggered alerts
    for (alert, handler_name) in alerts_triggered {
        // Check cooldown
        if !alert_manager.can_send(&alert) {
            warn!("Alert {} suppressed (cooldown)", handler_name);
            continue;
        }

        // Send to stdout
        if let Err(e) = stdout.send(&alert).await {
            error!("Failed to send stdout notification: {}", e);
        }

        // Send to Feishu if configured
        if let Some(feishu_handler) = feishu {
            if let Err(e) = feishu_handler.send(&alert).await {
                error!("Failed to send Feishu notification: {}", e);
            } else {
                info!("Feishu notification sent for alert: {}", handler_name);
            }
        }
    }
}

/// Create a default configuration for when config.toml is missing
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
