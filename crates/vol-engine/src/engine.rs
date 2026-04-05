//! Core monitoring engine - orchestrates datasources, rules, and notifications.

use vol_core::{DataSource, RuleProcessor, NotificationHandler, MonitoringEvent, Alert, error::Result};
use vol_alert::AlertManager;
use tokio::sync::{mpsc, broadcast};
use tokio::task::JoinHandle;
use tracing::{info, error, warn, debug, info_span};
use std::sync::Arc;
use vol_tracing::{record_tags, WithSpan};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use opentelemetry::trace::TraceContextExt;
use crate::config::EngineConfig;

/// Monitoring engine - the main event loop coordinator
pub struct MonitoringEngine {
    datasources: Vec<Box<dyn DataSource>>,
    rules: Vec<Box<dyn RuleProcessor>>,
    notifications: Vec<Box<dyn NotificationHandler>>,
    config: EngineConfig,
}

impl MonitoringEngine {
    /// Create a new engine with the given configuration
    pub fn new(config: EngineConfig) -> Self {
        Self {
            datasources: Vec::new(),
            rules: Vec::new(),
            notifications: Vec::new(),
            config,
        }
    }

    /// Register a datasource
    pub fn add_datasource(&mut self, ds: Box<dyn DataSource>) {
        info!("Registered datasource: {}", ds.name());
        self.datasources.push(ds);
    }

    /// Register a rule processor
    pub fn add_rule(&mut self, rule: Box<dyn RuleProcessor>) {
        info!("Registered rule: {} (interests: {:?})", rule.id(), rule.interests());
        self.rules.push(rule);
    }

    /// Register a notification handler
    pub fn add_notification(&mut self, notif: Box<dyn NotificationHandler>) {
        info!("Registered notification: {}", notif.name());
        self.notifications.push(notif);
    }

    /// Run the monitoring engine
    pub async fn run(self) -> Result<()> {
        info!("Starting monitoring engine...");
        info!("Datasources: {}", self.datasources.len());
        info!("Rules: {}", self.rules.len());
        info!("Notifications: {}", self.notifications.len());

        // Create channels
        // Use broadcast for events (multiple rules can subscribe)
        let (event_tx, _) = broadcast::channel::<WithSpan<MonitoringEvent>>(self.config.event_buffer_size);
        // Use mpsc for alerts (single queue for all notifications)
        let (alert_tx, alert_rx) = mpsc::channel::<Alert>(self.config.alert_buffer_size);

        // Spawn datasources - each runs independently
        let ds_handles = self.spawn_datasources(event_tx.clone());

        // Spawn rules - each rule gets its own broadcast subscription
        let rule_handles = self.spawn_rules(event_tx, alert_tx.clone());

        // Create AlertManager with config
        // AlertManager is created here in run() because it needs to be moved into
        // spawn_notifications, which takes ownership to transfer to the spawned task
        let alert_manager = AlertManager::new(self.config.config_file.clone());

        // Spawn notifications - single consumer for alerts
        let notif_handles = self.spawn_notifications(alert_rx, alert_manager);

        // Collect all handles
        let all_handles = ds_handles.into_iter()
            .chain(rule_handles)
            .chain(notif_handles);

        // Wait for first error or shutdown
        for handle in all_handles {
            if let Err(e) = handle.await {
                error!("Task failed: {:?}", e);
                break;
            }
        }

        info!("Monitoring engine stopped");
        Ok(())
    }

    fn spawn_datasources(
        &self,
        event_tx: broadcast::Sender<WithSpan<MonitoringEvent>>,
    ) -> Vec<JoinHandle<Result<()>>> {
        self.datasources
            .iter()
            .map(|ds| {
                let tx = event_tx.clone();
                let ds_clone = ds.clone_box();
                tokio::spawn(async move {
                    info!("Starting datasource: {}", ds_clone.name());
                    // Create mpsc channel for this datasource
                    let (ds_tx, mut ds_rx) = mpsc::channel::<MonitoringEvent>(100);

                    // Run datasource in a separate task
                    let ds_task = tokio::spawn(async move {
                        ds_clone.run(ds_tx).await
                    });

                    // Forward events to broadcast channel, wrapping each in WithSpan
                    while let Some(event) = ds_rx.recv().await {
                        // Create a new span for this event with business context
                        let span = info_span!(
                            "datasource_event",
                            source = %event.source(),
                            event_type = ?event.event_type()
                        );
                        span.record("timestamp", &event.timestamp());

                        // Wrap event with span for propagation to rules
                        let traced_event = WithSpan::new(event, span);

                        if tx.send(traced_event).is_err() {
                            warn!("No event receivers, stopping datasource");
                            break;
                        }
                    }

                    ds_task.await.unwrap_or(Ok(()))
                })
            })
            .collect()
    }

    fn spawn_rules(
        &self,
        event_tx: broadcast::Sender<WithSpan<MonitoringEvent>>,
        alert_tx: mpsc::Sender<Alert>,
    ) -> Vec<JoinHandle<Result<()>>> {
        self.rules
            .iter()
            .map(|rule| {
                let interests = rule.interests();
                let rule_id = rule.id().to_string();
                let rule_type = rule.rule_type().to_string();
                let mut rx = event_tx.subscribe();
                let tx = alert_tx.clone();
                let rule_clone = rule.clone_box_rule();
                tokio::spawn(async move {
                    info!("Starting rule: {}", rule_id);
                    while let Ok(traced_event) = rx.recv().await {
                        // Extract event and span from the wrapper
                        let (event, parent_span) = traced_event.split();

                        // Fast path: skip events we're not interested in
                        if !interests.contains(&event.event_type()) {
                            continue;
                        }

                        // Create span for rule evaluation with business attributes
                        let span = info_span!(
                            "rule_evaluate",
                            rule_id = %rule_id,
                            rule_type = %rule_type,
                            event_type = ?event.event_type()
                        );

                        // Establish causal relationship with parent span if present
                        if let Some(parent) = parent_span {
                            span.follows_from(parent.id());

                            // Inherit trace_id from parent for log correlation
                            let parent_ctx = parent.context();
                            let parent_trace_id = parent_ctx.span().span_context().trace_id();
                            span.record("parent_trace_id", &parent_trace_id.to_string());
                        }

                        // Record additional event-specific attributes
                        span.record("event.timestamp", &event.timestamp());
                        span.record("event.source", event.source());

                        // Evaluate rule within span context
                        let _guard = span.enter();
                        let alerts = rule_clone.evaluate(&event).await;
                        drop(_guard);

                        // Process each alert with its own span
                        for alert in alerts {
                            // Create child span for alert
                            let alert_span = info_span!(
                                "alert_generated",
                                alert_type = %alert.alert_type,
                                tenor = ?alert.tenor,
                                symbol = %alert.symbol
                            );

                            // Inherit trace_id from rule_evaluate span
                            let rule_trace_id = tracing::Span::current()
                                .context()
                                .span()
                                .span_context()
                                .trace_id();
                            alert_span.record("trace_id", &rule_trace_id.to_string());

                            // Record business attributes from Alert using record_tags! macro
                            record_tags!(alert_span, alert, iv, index_price, dte, moneyness, mark_price_coin);
                            alert_span.record("option_type", &alert.option_type.to_string());

                            // Send alert within span context (notification layer will add its own span)
                            if let Err(e) = tx.send(alert).await {
                                error!(error = %e, "Failed to send alert");
                                break;
                            }
                        }
                    }
                    Ok(())
                })
            })
            .collect()
    }

    fn spawn_notifications(
        &self,
        mut alert_rx: mpsc::Receiver<Alert>,
        alert_manager: AlertManager,
    ) -> Vec<JoinHandle<Result<()>>> {
        // For notifications, we use a fan-out pattern where each notification channel
        // runs in the same task to avoid needing mpsc resubscribe
        let notifications: Vec<Box<dyn NotificationHandler>> = self.notifications
            .iter()
            .filter(|n| n.is_enabled())
            .map(|n| n.clone_box())
            .collect();

        if notifications.is_empty() {
            return vec![];
        }

        let num_notifications = notifications.len();
        let alert_manager = Arc::new(alert_manager);
        vec![tokio::spawn(async move {
            info!("Starting {} notification channels", num_notifications);
            while let Some(alert) = alert_rx.recv().await {
                // Check cooldown before sending
                if !alert_manager.can_send(&alert) {
                    debug!("Alert in cooldown, skipping: {}:{}:{}",
                        alert.alert_type, alert.tenor, alert.symbol);
                    continue;
                }
                for notif in &notifications {
                    if let Err(e) = notif.send(&alert).await {
                        error!("Notification {} failed: {}", notif.name(), e);
                    }
                }
            }
            Ok(())
        })]
    }
}
