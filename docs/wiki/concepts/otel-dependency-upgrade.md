---
type: concept
category: infrastructure
tags: [otel, dependency, workspace, upgrade]
created: 2026-05-14
updated: 2026-05-14
source_count: 1
---

# OTel Dependency Upgrade

**Category:** Workspace dependency management

**Related:** [[otel-029-log-init]], [[otel-log-routing]], [[agent-observability]]

## Definition

Process for upgrading OpenTelemetry workspace dependencies from version 0.21 to 0.29, including breaking API changes and migration steps.

## Key Points

- **Version jump**: OTel 0.21 to 0.29 introduces significant breaking changes
- **API changes**: Builder patterns replaced nested config objects (`TracerProvider::builder().with_config()` flattened to `SdkTracerProvider::builder().with_sampler().with_resource()`)
- **Type renames**: `TracerProvider` renamed to `SdkTracerProvider`, new `SdkLoggerProvider` for logs
- **Exporter API**: `new_exporter().tonic()` replaced with type-specific `SpanExporter::builder().with_tonic()`
- **Logger bridge**: `OpenTelemetryTracingBridge` now requires 2 generic parameters (`P` and `L`)
- **Shutdown**: `global::shutdown_tracer_provider()` replaced with direct `provider.shutdown()` calls

## Migration Steps

1. Update workspace `Cargo.toml` versions
2. Migrate resource construction to builder pattern
3. Update tracer provider builder to flattened API
4. Replace exporter construction with type-specific builders
5. Add LoggerProvider initialization for log export
6. Update shutdown to use provider instance method

## Related Concepts
- [[otel-029-log-init]]: Implementation of OTel 0.29 migration in vol-monitor
- [[otel-log-routing]]: Architecture using upgraded OTel APIs
- [[agent-observability]]: Observability system affected by the upgrade
