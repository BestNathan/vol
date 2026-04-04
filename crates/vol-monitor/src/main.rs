//! vol-monitor: Main binary using channel-based engine.

mod state;
mod registry;

use anyhow::Result;
use tracing::{info, warn};
use tracing_subscriber::{self, EnvFilter};

use vol_config::{Config, DataSourceConfig, NotificationConfig, RuleConfig};
use vol_engine::{MonitoringEngineBuilder, EngineConfig};
use vol_datasource::{DeribitDataSource, PortfolioDataSource};
use vol_notification::{StdoutNotification, FeishuNotification};
use vol_rules::{AbsoluteIvRule, RateChangeRule, TermStructureRule, SkewRule, PortfolioRule};
use vol_config::{TermStructureConfig, SkewConfig};
use vol_deribit::DeribitClient;

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
    let engine_config = EngineConfig {
        event_buffer_size: config.engine.channel_buffer_size,
        alert_buffer_size: 100,
        enable_backpressure: true,
        config_file: config.engine.clone(),
    };

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
            DataSourceConfig::Portfolio(portfolio_cfg) => {
                if !portfolio_cfg.enabled {
                    continue;
                }

                // Find Deribit datasource config for auth credentials
                let deribit_auth = config.datasources.iter()
                    .find_map(|ds| {
                        if let DataSourceConfig::Deribit(d) = ds {
                            d.auth.clone()
                        } else {
                            None
                        }
                    });

                // Create DeribitClient with auth
                let mut client = DeribitClient::new("wss://www.deribit.com/ws/api/v2");
                if let Some(auth) = deribit_auth {
                    if let (Some(client_id), Some(client_secret)) = (auth.client_id(), auth.client_secret()) {
                        client = client.with_auth(client_id, client_secret);
                    }
                }

                let ds = PortfolioDataSource::new(
                    portfolio_cfg.id.clone(),
                    client,
                    portfolio_cfg.poll_interval_secs,
                    portfolio_cfg.currencies.clone(),
                );

                builder = builder.with_datasource(Box::new(ds));
                info!("Added portfolio datasource: {}", portfolio_cfg.id);
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
                match FeishuNotification::new(feishu_config) {
                    Ok(feishu) => {
                        builder = builder.with_notification(Box::new(feishu));
                        info!("Added notification: {}", feishu_cfg.id);
                    }
                    Err(e) => {
                        warn!("Failed to initialize Feishu notification: {:?}", e);
                    }
                }
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
            RuleConfig::Portfolio(portfolio_cfg) => {
                if !portfolio_cfg.enabled {
                    continue;
                }

                let rule = PortfolioRule::new(
                    portfolio_cfg.metrics.clone(),
                    config.engine.alert_cooldown_secs,
                    portfolio_cfg.id.clone(),
                    portfolio_cfg.notifications.clone(),
                );

                builder = builder.with_rule(Box::new(rule));
                info!("Added portfolio rule: {} (metrics: {})",
                    portfolio_cfg.id,
                    portfolio_cfg.metrics.iter().filter(|m| m.enabled()).count());
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
            tenor_cooldowns: vol_config::TenorCooldownsConfig {
                short_secs: Some(600),    // 10 minutes
                medium_secs: Some(3600),  // 1 hour
                long_secs: Some(14400),   // 4 hours
            },
        },
        tenors: vol_config::TenorConfig {
            short_max_dte: 7,
            medium_min_dte: 20,
            medium_max_dte: 40,
            long_min_dte: 80,
            long_max_dte: 200,
        },
        datasources: vec![],
        notifications: vec![],
        rules: vec![],
        data_sources: None,
        alerts: None,
        state: None,
    }
}
