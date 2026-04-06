//! vol-monitor: Main binary using channel-based engine.

mod state;
mod registry;
mod tracing_setup;

use anyhow::Result;
use tracing::{info, warn};

use vol_config::{Config, DataSourceConfig, NotificationConfig, RuleConfig};
use vol_engine::{MonitoringEngineBuilder, EngineConfig};
use vol_datasource::{VolatilityDataSource, PortfolioDataSource};
use vol_notification::{StdoutNotification, FeishuNotification};
use vol_rules::{AbsoluteIvRule, RateChangeRule, TermStructureRule, SkewRule, PortfolioRule};
use vol_config::{TermStructureConfig, SkewConfig};

/// Parse command line arguments
fn parse_args() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                if i + 1 < args.len() {
                    config_path = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --config requires a file path");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                println!("vol-monitor - Deribit Volatility Monitor");
                println!();
                println!("Usage: vol-monitor [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -c, --config <FILE>  Configuration file path (default: config.toml)");
                println!("  -h, --help           Print this help message");
                println!();
                println!("Environment variables:");
                println!("  DERIBIT_CLIENT_ID      Deribit API client ID");
                println!("  DERIBIT_CLIENT_SECRET  Deribit API client secret");
                println!("  FEISHU_APP_ID          Feishu app ID");
                println!("  FEISHU_APP_SECRET      Feishu app secret");
                println!("  FEISHU_RECEIVE_ID      Feishu message recipient ID");
                println!("  HTTPS_PROXY            HTTP proxy for API calls");
                println!("  RUST_LOG               Log level filter");
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    config_path
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let config_path = parse_args().unwrap_or_else(|| "config.toml".to_string());

    // Load configuration
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        warn!("Failed to load {}: {}", config_path, e);
        warn!("Using default configuration");
        create_default_config()
    });

    // Initialize tracing and logging
    if let Err(e) = tracing_setup::init(&config.tracing) {
        tracing::error!("Failed to initialize tracing: {}", e);
    }

    info!("===========================================");
    info!("  Deribit Volatility Monitor v0.3.0");
    info!("===========================================");

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
            DataSourceConfig::Volatility(vol_cfg) => {
                // Get Deribit client config
                let deribit_config = config.clients.deribit.clone()
                    .expect("VolatilityDataSource requires [clients.deribit] configuration");

                let ds = VolatilityDataSource::from_config(
                    deribit_config,
                    vol_cfg.symbols.clone(),
                    vol_cfg.id.clone(),
                );

                builder = builder.with_datasource(Box::new(ds));
                info!("Added datasource: {} (type: volatility, symbols: {:?})", vol_cfg.id, vol_cfg.symbols);
            }
            DataSourceConfig::Portfolio(portfolio_cfg) => {
                // Get Deribit client config
                let deribit_config = config.clients.deribit.clone()
                    .expect("PortfolioDataSource requires [clients.deribit] configuration");

                let ds = PortfolioDataSource::from_config(
                    deribit_config,
                    portfolio_cfg.clone(),
                );

                builder = builder.with_datasource(Box::new(ds));
                info!("Added datasource: {} (type: portfolio, currencies: {:?})", portfolio_cfg.id, portfolio_cfg.currencies);
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
                // Use environment variable methods from FeishuNotificationConfig
                let feishu_config = vol_config::FeishuConfig {
                    app_id: Some(feishu_cfg.app_id()),
                    app_secret: Some(feishu_cfg.app_secret()),
                    receive_id: Some(feishu_cfg.receive_id()),
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

    // Shutdown tracing
    tracing_setup::shutdown();

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
        clients: vol_config::ClientConfigs::default(),
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
        tracing: vol_config::TracingConfig::default(),
        data_sources: None,
        alerts: None,
        state: None,
    }
}
