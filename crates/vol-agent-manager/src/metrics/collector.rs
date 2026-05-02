use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry,
    register_gauge_with_registry, register_histogram_vec_with_registry, HistogramVec, Registry,
};
use prometheus::{CounterVec, Gauge, GaugeVec};

/// Prometheus metrics for the agent manager.
pub struct MetricsCollector {
    pub registry: Registry,
    pub agent_connections_current: Gauge,
    pub agent_registered_total: Gauge,
    pub agent_messages_total: CounterVec,
    pub agent_status_count: GaugeVec,
    pub agent_metric_samples_total: CounterVec,
    pub agent_heartbeat_latency_seconds: HistogramVec,
    pub agent_task_duration_seconds: HistogramVec,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Registry::new_custom(Some("agent_manager".to_string()), None).unwrap();

        let agent_connections_current =
            register_gauge_with_registry!(
                "agent_connections_current",
                "Current active WebSocket connections",
                registry
            )
            .unwrap();

        let agent_registered_total =
            register_gauge_with_registry!(
                "agent_registered_total",
                "Total registered agents",
                registry
            )
            .unwrap();

        let agent_messages_total = register_counter_vec_with_registry!(
            "agent_messages_total",
            "Total messages received by type",
            &["message_type", "agent_id", "agent_type"],
            registry
        )
        .unwrap();

        let agent_status_count = register_gauge_vec_with_registry!(
            "agent_status_count",
            "Count of agents in each status",
            &["status"],
            registry
        )
        .unwrap();

        let agent_metric_samples_total = register_counter_vec_with_registry!(
            "agent_metric_samples_total",
            "Total metric samples received",
            &["agent_id"],
            registry
        )
        .unwrap();

        let agent_heartbeat_latency_seconds = register_histogram_vec_with_registry!(
            "agent_heartbeat_latency_seconds",
            "Heartbeat round-trip latency",
            &["agent_id", "agent_type"],
            registry
        )
        .unwrap();

        let agent_task_duration_seconds = register_histogram_vec_with_registry!(
            "agent_task_duration_seconds",
            "Task execution duration",
            &["task_type", "agent_id", "status"],
            registry
        )
        .unwrap();

        Self {
            registry,
            agent_connections_current,
            agent_registered_total,
            agent_messages_total,
            agent_status_count,
            agent_metric_samples_total,
            agent_heartbeat_latency_seconds,
            agent_task_duration_seconds,
        }
    }

    /// Increment message counter.
    pub fn increment_messages(&self, message_type: &str, agent_id: &str, agent_type: &str) {
        self.agent_messages_total
            .with_label_values(&[message_type, agent_id, agent_type])
            .inc();
    }

    /// Increment metric samples counter.
    pub fn increment_metric_samples(&self, agent_id: &str) {
        self.agent_metric_samples_total
            .with_label_values(&[agent_id])
            .inc();
    }

    /// Observe heartbeat latency.
    pub fn observe_heartbeat_latency(&self, agent_id: &str, agent_type: &str, seconds: f64) {
        self.agent_heartbeat_latency_seconds
            .with_label_values(&[agent_id, agent_type])
            .observe(seconds);
    }

    /// Observe task duration.
    pub fn observe_task_duration(
        &self,
        task_type: &str,
        agent_id: &str,
        status: &str,
        seconds: f64,
    ) {
        self.agent_task_duration_seconds
            .with_label_values(&[task_type, agent_id, status])
            .observe(seconds);
    }

    /// Gather all metrics for /metrics endpoint.
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let mc = MetricsCollector::new();
        let output = mc.gather();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_increment_connections() {
        let mc = MetricsCollector::new();
        mc.agent_connections_current.set(1.0);
        assert_eq!(mc.agent_connections_current.get() as i64, 1);
    }

    #[test]
    fn test_increment_messages() {
        let mc = MetricsCollector::new();
        mc.increment_messages("heartbeat", "agent-1", "react-agent");
        assert!(
            mc.gather().iter().any(|m| m.get_name() == "agent_manager_agent_messages_total")
        );
    }
}
