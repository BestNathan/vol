//! Core monitoring engine - orchestrates datasources, rules, and notifications.

use crate::config::EngineConfig;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, info_span, warn};
use vol_alert::AlertManager;
use vol_core::{
    error::Result, Alert, DataSource, MonitoringEvent, NotificationHandler, RuleProcessor,
};
use vol_tracing::{Instrument, TracedEvent};

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
        info!(
            "Registered rule: {} (interests: {:?})",
            rule.id(),
            rule.interests()
        );
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
        let (event_tx, _) =
            broadcast::channel::<TracedEvent<MonitoringEvent>>(self.config.event_buffer_size);
        // Use broadcast for alerts (multiple subscribers: notifications + AgentAdviceService)
        let (alert_tx, _) = broadcast::channel::<TracedEvent<Alert>>(self.config.alert_buffer_size);

        // Spawn datasources - each runs independently
        let ds_handles = self.spawn_datasources(event_tx.clone());

        // Spawn rules - each rule gets its own broadcast subscription
        let rule_handles = self.spawn_rules(event_tx, alert_tx.clone());

        // Create AlertManager with config
        // AlertManager is created here in run() because it needs to be moved into
        // spawn_notifications, which takes ownership to transfer to the spawned task
        let alert_manager = Arc::new(AlertManager::new(self.config.config_file.clone()));

        // Spawn notifications - subscribe to broadcast for alerts
        let notif_handles = self.spawn_notifications(alert_tx.subscribe(), alert_manager);

        // Collect all handles
        let all_handles = ds_handles
            .into_iter()
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
        event_tx: broadcast::Sender<TracedEvent<MonitoringEvent>>,
    ) -> Vec<JoinHandle<Result<()>>> {
        self.datasources
            .iter()
            .map(|ds| {
                let tx = event_tx.clone();
                let ds_clone = ds.clone_box();
                tokio::spawn(async move {
                    info!("Starting datasource: {}", ds_clone.name());
                    // Create mpsc channel for this datasource - now carries TracedEvent for tracing context
                    let (ds_tx, mut ds_rx) = mpsc::channel::<TracedEvent<MonitoringEvent>>(100);

                    // Run datasource in a separate task
                    let ds_task = tokio::spawn(async move { ds_clone.run(ds_tx).await });

                    // Forward events to broadcast channel
                    while let Some(traced_event) = ds_rx.recv().await {
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
        event_tx: broadcast::Sender<TracedEvent<MonitoringEvent>>,
        alert_tx: broadcast::Sender<TracedEvent<Alert>>,
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
                        // Extract event, parent_span, and trace_id from the wrapper
                        let (event, parent_span, trace_id) = traced_event.split();

                        // Fast path: skip events we're not interested in
                        if !interests.contains(&event.event_type()) {
                            continue;
                        }

                        // Create span for rule evaluation with business attributes
                        let span = info_span!(
                            "rule_evaluate",
                            rule_id = %rule_id,
                            rule_type = %rule_type,
                            event_type = ?event.event_type(),
                            event_timestamp = %event.timestamp(),
                            event_source = %event.source(),
                            trace_id = %trace_id,
                        );

                        // Establish causal relationship with parent span from datasource
                        if let Some(parent) = parent_span {
                            span.follows_from(parent.id());
                        }

                        // Evaluate rule within span context
                        let alerts = rule_clone.evaluate(&event).instrument(span.clone()).await;

                        // Process each alert with its own span
                        for alert in alerts {
                            // Create child span for alert with same trace_id
                            let alert_span = info_span!(
                                "alert_generated",
                                alert_type = %alert.alert_type,
                                tenor = ?alert.tenor,
                                symbol = %alert.symbol,
                                iv = %alert.iv,
                                dte = alert.dte,
                                index_price = %alert.index_price,
                                trace_id = %trace_id,
                            );

                            // Establish causal relationship with rule_evaluate span
                            alert_span.follows_from(span.id());

                            // Wrap alert with span and trace_id for notification layer
                            let traced_alert =
                                TracedEvent::new(alert, alert_span.clone(), trace_id.clone());

                            // Send alert within span context
                            // broadcast::send is synchronous (returns Result<usize, SendError>)
                            // usize is the number of receivers that successfully received
                            // Error only when no receivers are subscribed (shouldn't happen in normal operation)
                            let _span = alert_span.enter();
                            if let Err(e) = tx.send(traced_alert) {
                                error!(error = %e, "Failed to broadcast alert (no receivers)");
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
        alert_rx: broadcast::Receiver<TracedEvent<Alert>>,
        alert_manager: Arc<AlertManager>,
    ) -> Vec<JoinHandle<Result<()>>> {
        // For notifications, we use a fan-out pattern where each notification channel
        // runs in the same task to avoid needing mpsc resubscribe
        let notifications: Vec<Box<dyn NotificationHandler>> = self
            .notifications
            .iter()
            .filter(|n| n.is_enabled())
            .map(|n| n.clone_box())
            .collect();

        if notifications.is_empty() {
            return vec![];
        }

        let num_notifications = notifications.len();
        vec![tokio::spawn(async move {
            info!("Starting {} notification channels", num_notifications);
            let mut rx = alert_rx;
            while let Ok(traced_alert) = rx.recv().await {
                // Extract alert, span, and trace_id from the wrapper
                let (mut alert, parent_span, trace_id) = traced_alert.split();

                // Set trace_id on alert for notification layer to use
                alert.trace_id = trace_id.clone();

                // Create notification span with the same trace_id
                let notif_span = info_span!(
                    "notification_send",
                    trace_id = %trace_id,
                    alert_type = %alert.alert_type,
                    channel = "stdout"
                );

                // Establish causal relationship with alert_generated span
                if let Some(parent) = parent_span {
                    notif_span.follows_from(parent.id());
                }

                // Check cooldown before sending
                if !alert_manager.can_send(&alert) {
                    debug!(parent: &notif_span, "Alert in cooldown, skipping: {}:{}:{}",
                        alert.alert_type, alert.tenor, alert.symbol);
                    continue;
                }

                // Send to each notification channel within span context
                for notif in &notifications {
                    if let Err(e) = notif.send(&alert).instrument(notif_span.clone()).await {
                        error!(parent: &notif_span, "Notification {} failed: {}", notif.name(), e);
                    }
                }
            }
            Ok(())
        })]
    }
}
