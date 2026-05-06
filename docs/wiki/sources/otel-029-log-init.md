---
type: source
category: development-task
tags: [otel, tracing, migration, vol-monitor, api-upgrade]
created: 2026-05-06
updated: 2026-05-06
---

# OTel 0.29 Migration and Log Initialization in vol-monitor

## Summary

Task 8 of the OTel migration pipeline: updated `crates/vol-monitor/src/tracing_setup.rs` from OTel 0.21 to 0.29 APIs and added `init_otel_logs()` function for OTel log export.

## API Changes Applied

### Resource Construction
- **Old (0.21):** `Resource::new(vec![KeyValue::new(...), ...])`
- **New (0.29):** `Resource::builder().with_service_name(...).with_attributes([...]).build()`

### TracerProvider Builder
- **Old (0.21):** `TracerProvider::builder().with_config(trace::Config::default().with_sampler(...).with_resource(...))`
- **New (0.29):** `SdkTracerProvider::builder().with_sampler(...).with_resource(...).with_batch_exporter(exporter).build()`
- Type renamed: `TracerProvider` -> `SdkTracerProvider` in `opentelemetry_sdk::trace`

### Exporter Construction
- **Old (0.21):** `opentelemetry_otlp::new_exporter().tonic().with_endpoint(...).build_span_exporter()`
- **New (0.29):** `opentelemetry_otlp::SpanExporter::builder().with_tonic().with_endpoint(...).build()`
- Batch exporter no longer takes runtime parameter: `with_batch_exporter(exporter)` instead of `with_batch_exporter(exporter, runtime::Tokio)`

### LoggerProvider (new)
- `SdkLoggerProvider::builder().with_resource(...).with_batch_exporter(exporter).build()`
- `LogExporter::builder().with_tonic().with_endpoint(...).build()`

### Global Shutdown
- **Old:** `global::shutdown_tracer_provider()`
- **New:** Call `provider.shutdown()` directly on the stored `SdkTracerProvider` instance

### OpenTelemetryTracingBridge
- Now takes 2 generic parameters: `OpenTelemetryTracingBridge<SdkLoggerProvider, SdkLogger>`

## init_otel_logs() Function

New public function that:
1. Builds an `OtelLogExporter` using `LogExporter::builder().with_tonic()`
2. Creates a `SdkLoggerProvider` with resource attributes and batch export
3. Returns an `OpenTelemetryTracingBridge` layer for integration into the tracing subscriber
4. Falls back gracefully with a warning if initialization fails

## Integration

The `init()` function now conditionally adds the OTel log layer in both the OTel-enabled and non-OTel branches:
- **OTel enabled:** `init_otel_logs()` is called; on failure, logs a warning and continues without the log layer
- **OTel disabled:** Log layer is `None` (type alias `OtelLogLayer` used for type inference)

## Files Changed
- `crates/vol-monitor/Cargo.toml` — added `opentelemetry-appender-tracing = { workspace = true }`
- `crates/vol-monitor/src/tracing_setup.rs` — full rewrite of OTel initialization

## Related
- [[otel-log-routing]]: Initialization flow now matches the 0.29 API
- [[agent-observability]]: OTel log layer is the bridge from local tracing to OTel Collector
- [[otel-dependency-upgrade]]: Workspace dependency upgrade context
