# Architecture Overview

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Kubernetes Cluster                      │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                 vol-monitor Deployment                  │ │
│  │                                                         │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │              Monitoring Engine                     │  │ │
│  │  │                                                   │  │ │
│  │  │  ┌─────────────┐  ┌─────────────┐  ┌───────────┐ │  │ │
│  │  │  │ Datasources │─▶│   Rules     │─▶│Notifications│ │  │ │
│  │  │  │             │  │             │  │           │ │  │ │
│  │  │  └─────────────┘  └─────────────┘  └───────────┘ │  │ │
│  │  │       │                  │               │        │  │ │
│  │  │       └──────────────────┼───────────────┘        │  │ │
│  │  │                          │                        │  │ │
│  │  │              TracedEvent<T>                        │  │ │
│  │  │              (trace context)                       │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  │                                                         │ │
│  │  Config: ConfigMap    Secrets: vol-monitor-secrets     │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ WebSocket
                              ▼
                    ┌─────────────────┐
                    │   Deribit API   │
                    │   (Market Data) │
                    └─────────────────┘
```

## Data Flow

```
Deribit WebSocket
       │
       ▼
┌─────────────────┐
│ VolatilityData  │
│ source          │
└────────┬────────┘
         │
         │ mpsc channel
         │ TracedEvent<VolatilityData>
         │ - trace_id
         │ - parent_span
         ▼
┌─────────────────┐
│ MonitoringEngine│
│ Event Loop      │
└────────┬────────┘
         │
         │ broadcast channel
         │ TracedEvent<MonitoringEvent>
         ▼
┌─────────────────┐
│ Rule Processors │
│ (per-rule eval) │
└────────┬────────┘
         │
         │ mpsc channel
         │ TracedEvent<Alert>
         ▼
┌─────────────────┐
│ Notification    │
│ Handlers        │
└─────────────────┘
```

## Key Design Patterns

| Pattern | Description |
|---------|-------------|
| **Trait-based plugin architecture** | All extension points use traits from `vol-core` |
| **Async-first** | All crates use tokio; no blocking I/O |
| **Channel-based communication** | `tokio::mpsc` and `broadcast` for component communication |
| **Trace context propagation** | `TracedEvent<T>` wrapper for cross-channel tracing |
| **State persistence** | Alert state saved on shutdown |

## Crate Organization

See [crates.md](crates.md) for detailed crate documentation.

| Crate | Purpose |
|-------|---------|
| `vol-core` | Shared traits and data models |
| `vol-config` | TOML-based configuration loading |
| `vol-tracing` | Tracing utilities and span helpers |
| `vol-deribit` | Deribit client and types |
| `vol-datasource` | Data providers |
| `vol-alert` | Alert evaluation logic |
| `vol-rules` | Rule processors |
| `vol-notification` | Alert delivery |
| `vol-engine` | Monitoring engine orchestration |
| `vol-monitor` | Main binary |
