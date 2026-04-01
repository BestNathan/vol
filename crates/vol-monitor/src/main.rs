//! vol-monitor: Main binary using channel-based engine.

mod state;
mod registry;

use anyhow::Result;
use tracing::{info, warn};
use tracing_subscriber::{self, EnvFilter};

use vol_config::{Config, DataSourceConfig, NotificationConfig, RuleConfig};
use vol_engine::{MonitoringEngineBuilder, EngineConfig};
use vol_datasource::DeribitDataSource;
use vol_notification::{StdoutNotification, FeishuNotification};
use vol_rules::{AbsoluteIvRule, RateChangeRule, TermStructureRule, SkewRule};
use vol_config::{TermStructureConfig, SkewConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("vol_monitor=info".parse().unwrap()))
        .init();

    info!("===========================================");
    info!("  Deribit Volatility Monitor v0.3.0");
    info!("===========================================");

    // Load configuration
    let config = Config::load("config.toml").unwrap_or_else(|e| {
        warn!("Failed to load config.toml: {}", e);
        warn!("Using default configuration");
        create_default_config()
    });

    // Create engine config
    let engine_config = EngineConfig::default();

    // Build engine
    let mut builder = MonitoringEngineBuilder::new()
        .with_config(engine_config);

    // Add datasources
    for ds_config in &config.datasources {
        match ds_config {
            DataSourceConfig::Deribit(deribit_cfg) => {
                if !deribit_cfg.enabled {
                    continue;
                }

                let mut ds = DeribitDataSource::new(
                    deribit_cfg.ws_url.clone(),
                    deribit_cfg.symbols.clone(),
                    deribit_cfg.poll_interval_secs,
                    deribit_cfg.id.clone(),
                );

                // Use proxy if configured
                if let Ok(proxy) = std::env::var("HTTPS_PROXY").or_else(|_| std::env::var("HTTP_PROXY")) {
                    info!("Using proxy: {}", proxy);
                    ds = ds.with_proxy(proxy);
                }

                builder = builder.with_datasource(Box::new(ds));
                info!("Added datasource: {} (provider: deribit)", deribit_cfg.id);
            }
            DataSourceConfig::Binance(_) => {
                warn!("Binance datasource not yet implemented");
            }
            DataSourceConfig::Internal(_) => {
                warn!("Internal datasource not yet implemented");
            }
        }
    }

    // Add notifications
    for notif_config in &config.notifications {
        match notif_config {
            NotificationConfig::Stdout(stdout_cfg) => {
                if !stdout_cfg.enabled {
                    continue;
                }
                let stdout = StdoutNotification::new();
                builder = builder.with_notification(Box::new(stdout));
                info!("Added notification: {}", stdout_cfg.id);
            }
            NotificationConfig::Feishu(feishu_cfg) => {
                if !feishu_cfg.enabled {
                    continue;
                }
                let feishu_config = vol_config::FeishuConfig {
                    app_id: Some(feishu_cfg.app_id.clone()),
                    app_secret: Some(feishu_cfg.app_secret.clone()),
                    receive_id: Some(feishu_cfg.receive_id.clone()),
                    message_template: feishu_cfg.message_template.clone(),
                };
                let feishu = FeishuNotification::new(feishu_config);
                builder = builder.with_notification(Box::new(feishu));
                info!("Added notification: {}", feishu_cfg.id);
            }
        }
    }

    // Add rules
    for rule_config in &config.rules {
        if !rule_config.enabled() {
            continue;
        }

        match rule_config {
            RuleConfig::AbsoluteIv(abs_cfg) => {
                let rule = AbsoluteIvRule::new(abs_cfg.clone());
                builder = builder.with_rule(Box::new(rule));
                info!("Added rule: {} (type: absolute-iv, symbol: {})", abs_cfg.id, abs_cfg.symbol);
            }
            RuleConfig::RateChange(rate_cfg) => {
                let rule = RateChangeRule::new(rate_cfg.clone());
                builder = builder.with_rule(Box::new(rule));
                info!("Added rule: {} (type: rate-change, symbol: {})", rate_cfg.id, rate_cfg.symbol);
            }
            RuleConfig::TermStructure(term_cfg) => {
                let config = TermStructureConfig {
                    short_long_spread_threshold: term_cfg.short_long_spread_threshold,
                };
                let rule = TermStructureRule::new(config, term_cfg.id.clone());
                builder = builder.with_rule(Box::new(rule));
                info!("Added rule: {} (type: term-structure)", term_cfg.id);
            }
            RuleConfig::Skew(skew_cfg) => {
                let config = SkewConfig {
                    threshold: skew_cfg.threshold,
                };
                let rule = SkewRule::new(config, skew_cfg.id.clone());
                builder = builder.with_rule(Box::new(rule));
                info!("Added rule: {} (type: skew)", skew_cfg.id);
            }
            RuleConfig::Portfolio(_) => {
                warn!("Portfolio rule not yet implemented in new format");
            }
            RuleConfig::MarginRatio(_) => {
                warn!("MarginRatio rule not yet implemented");
            }
        }
    }

    let engine = builder.build();

    info!("===========================================");
    info!("  Monitoring started");
    info!("===========================================");
    info!("");
    info!("Configuration loaded:");
    info!("  Datasources: {}", config.datasources.len());
    info!("  Notifications: {}", config.notifications.len());
    info!("  Rules: {}", config.rules.iter().filter(|r| r.enabled()).count());
    info!("");

    // Run engine (runs until shutdown)
    // TODO: Handle shutdown signal and save state
    let _ = engine.run().await;

    Ok(())
}

fn create_default_config() -> Config {
    Config {
        engine: vol_config::EngineConfigFile {
            hot_reload: false,
            hot_reload_interval_secs: 30,
            channel_buffer_size: 1000,
            alert_cooldown_secs: 300,
        },
        tenors: vol_config::TenorConfig {
            short_max_dte: 7,
            medium_min_dte: 20,
            medium_max_dte: 40,
            long_min_dte: 80,
        },
        datasources: vec![],
        notifications: vec![],
        rules: vec![],
        data_sources: None,
        alerts: None,
        state: None,
    }
}
