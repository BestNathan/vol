//! Core monitoring engine - orchestrates datasources, rules, and notifications.

use vol_core::{DataSource, RuleProcessor, NotificationChannel, MonitoringEvent, Alert, error::Result};
use tokio::sync::{mpsc, broadcast};
use tokio::task::JoinHandle;
use tracing::{info, error, warn};
use crate::config::EngineConfig;

/// Monitoring engine - the main event loop coordinator
pub struct MonitoringEngine {
    datasources: Vec<Box<dyn DataSource>>,
    rules: Vec<Box<dyn RuleProcessor>>,
    notifications: Vec<Box<dyn NotificationChannel>>,
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
        info!("Registered rule: {} (interests: {:?})", rule.name(), rule.interests());
        self.rules.push(rule);
    }

    /// Register a notification channel
    pub fn add_notification(&mut self, notif: Box<dyn NotificationChannel>) {
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
        let (event_tx, _) = broadcast::channel::<MonitoringEvent>(self.config.event_buffer_size);
        // Use mpsc for alerts (single queue for all notifications)
        let (alert_tx, alert_rx) = mpsc::channel::<Alert>(self.config.alert_buffer_size);

        // Spawn datasources - each runs independently
        let ds_handles = self.spawn_datasources(event_tx.clone());

        // Spawn rules - each rule gets its own broadcast subscription
        let rule_handles = self.spawn_rules(event_tx, alert_tx.clone());

        // Spawn notifications - single consumer for alerts
        let notif_handles = self.spawn_notifications(alert_rx);

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
        event_tx: broadcast::Sender<MonitoringEvent>,
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

                    // Forward events to broadcast channel
                    while let Some(event) = ds_rx.recv().await {
                        if tx.send(event).is_err() {
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
        event_tx: broadcast::Sender<MonitoringEvent>,
        alert_tx: mpsc::Sender<Alert>,
    ) -> Vec<JoinHandle<Result<()>>> {
        self.rules
            .iter()
            .map(|rule| {
                let interests = rule.interests();
                let mut rx = event_tx.subscribe();
                let tx = alert_tx.clone();
                let rule_clone = rule.clone_box_rule();
                tokio::spawn(async move {
                    info!("Starting rule: {}", rule_clone.name());
                    while let Ok(event) = rx.recv().await {
                        // Fast path: skip events we're not interested in
                        if !interests.contains(&event.event_type()) {
                            continue;
                        }

                        if let Some(alert) = rule_clone.evaluate(&event) {
                            if let Err(e) = tx.send(alert).await {
                                error!("Failed to send alert: {}", e);
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
    ) -> Vec<JoinHandle<Result<()>>> {
        // For notifications, we use a fan-out pattern where each notification channel
        // runs in the same task to avoid needing mpsc resubscribe
        let notifications: Vec<Box<dyn NotificationChannel>> = self.notifications
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
            while let Some(alert) = alert_rx.recv().await {
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
