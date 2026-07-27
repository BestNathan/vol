//! Shared Prometheus registry + axum /metrics endpoint.
//!
//! This module provides a process-wide [`prometheus::Registry`] that both the
//! OTel Prometheus exporter (writer) and the HTTP /metrics handler (reader)
//! share.  This ensures the handler reads the SAME metrics the exporter writes,
//! avoiding the pitfall of two different prometheus crate versions (0.13 vs
//! 0.14) with separate, incompatible registries.

use std::sync::OnceLock;

use axum::{http::StatusCode, routing::get, Router};
use prometheus::Registry;

static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Get (or initialize) the process-wide Prometheus registry used by the
/// OTel Prometheus exporter.  Both `otel_init` (writer) and the `/metrics`
/// handler (reader) use this same instance.
pub fn registry() -> &'static Registry {
    REGISTRY.get_or_init(Registry::new)
}

/// Build an axum Router exposing GET /metrics in Prometheus text format.
pub fn build_metrics_router() -> Router {
    Router::new().route("/metrics", get(metrics_handler))
}

async fn metrics_handler() -> Result<String, StatusCode> {
    use prometheus::TextEncoder;
    let metric_families = registry().gather();
    TextEncoder::new()
        .encode_to_string(&metric_families)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_gather_includes_registered_metric() {
        let reg = Registry::new();
        let counter = prometheus::IntCounter::new("test_total", "test").unwrap();
        reg.register(Box::new(counter.clone())).unwrap();
        counter.inc();
        let mfs = reg.gather();
        let out = prometheus::TextEncoder::new()
            .encode_to_string(&mfs)
            .unwrap();
        assert!(out.contains("test_total"));
    }

    #[test]
    fn test_build_metrics_router_constructs() {
        let _router = build_metrics_router();
    }
}
