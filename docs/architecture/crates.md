# Workspace Structure

## Overview

This is a Cargo workspace with 10 crates organized in a modular architecture.

## Crate Organization

| Crate | Purpose | Dependencies |
|-------|---------|--------------|
| `vol-core` | Shared traits and data models | - |
| `vol-config` | TOML-based configuration loading | vol-core |
| `vol-tracing` | Tracing utilities and span helpers | - |
| `vol-deribit` | Deribit client and types | - |
| `vol-datasource` | Data providers (Deribit, Portfolio) | vol-core, vol-deribit |
| `vol-alert` | Alert evaluation logic | vol-core, vol-config |
| `vol-rules` | Rule processors | vol-core, vol-config, vol-alert |
| `vol-notification` | Alert delivery (stdout, Feishu) | vol-core, vol-config |
| `vol-engine` | Monitoring engine (orchestration) | vol-core, vol-alert, vol-config |
| `vol-monitor` | Main binary | All crates |

## Crate Details

### vol-core

**Path:** `crates/vol-core/src/`

Core traits and types used across the system:
- `DataSource` - Data source interface
- `RuleProcessor` - Rule evaluation interface
- `NotificationHandler` - Notification delivery interface
- `VolatilityData` - Market data model
- `Alert` - Alert data model
- `Tenor` - Short/Medium/Long tenor classification
- `VolError` - Error types

### vol-config

**Path:** `crates/vol-config/src/`

Configuration loading and parsing:
- `Config` - Main configuration structure
- `EngineConfigFile` - Engine settings
- `DeribitClientConfig` - Deribit connection settings
- `DataSourceConfig` - Data source configuration
- `NotificationConfig` - Notification channel configuration
- `RuleConfig` - Rule configuration types

### vol-tracing

**Path:** `crates/vol-tracing/src/`

Tracing and observability utilities:
- `TracedEvent<T>` - Wrapper for trace context propagation
- `new_trace_id()` - Generate UUID v4 trace ID
- `current_trace_id()` - Get current trace context
- `WithSpan` - Span wrapper (legacy)
- `instrument` macro - Function instrumentation

### vol-deribit

**Path:** `crates/vol-deribit/src/`

Deribit-specific client and types:
- `DeribitClient` - WebSocket client with auto-reconnect
- `ChannelType` - Subscription channel types
- `ChannelData` - Market data types
- `OptionMarkPrice` - Option mark price data
- `DeribitTicker` - Ticker data
- `Instrument` parsing - Instrument name parsing and DTE calculation

### vol-datasource

**Path:** `crates/vol-datasource/src/`

Data source implementations:
- `VolatilityDataSource` - Deribit WebSocket data (mark prices, IV)
- `PortfolioDataSource` - Account positions and Greeks
- Data merging and index price state management

### vol-alert

**Path:** `crates/vol-alert/src/`

Alert evaluation logic:
- `AbsoluteIvRule` - IV threshold alerts
- `RateChangeRule` - Rapid IV change detection (1h/4h/24h)
- `TermStructureRule` - Short-long spread anomalies
- `SkewRule` - Call-put skew detection
- `PortfolioRule` - Portfolio Greeks and PnL monitoring

### vol-rules

**Path:** `crates/vol-rules/src/`

Rule processors that combine alert logic with configuration.

### vol-notification

**Path:** `crates/vol-notification/src/`

Notification delivery:
- `StdoutNotification` - Console output
- `FeishuNotification` - Feishu/Lark integration with interactive cards

### vol-engine

**Path:** `crates/vol-engine/src/`

Monitoring engine orchestration:
- `MonitoringEngine` - Main event loop coordinator
- Channel management (broadcast for events, mpsc for alerts)
- Task spawning for datasources, rules, and notifications
- Trace context propagation across components

### vol-monitor

**Path:** `crates/vol-monitor/src/`

Main binary:
- Configuration loading
- Engine setup and wiring
- CLI argument parsing
- Tracing/logging initialization

## Data Flow

```
┌─────────────────┐
│ Deribit         │
│ WebSocket       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ VolatilityData  │
│ source          │
└────────┬────────┘
         │
         │ mpsc channel
         │ (TracedEvent<VolatilityData>)
         ▼
┌─────────────────┐
│ MonitoringEngine│
│ Event Loop      │
└────────┬────────┘
         │
         │ broadcast channel
         │ (TracedEvent<MonitoringEvent>)
         ▼
┌─────────────────┐
│ Rule Processors │
│ (per-rule eval) │
└────────┬────────┘
         │
         │ mpsc channel
         │ (TracedEvent<Alert>)
         ▼
┌─────────────────┐
│ Notification    │
│ Handlers        │
└─────────────────┘
```

## Key Design Patterns

- **Trait-based plugin architecture**: All extension points use traits from `vol-core`
- **Async-first**: All crates use tokio; no blocking I/O
- **Channel-based communication**: `tokio::mpsc` and `broadcast` for component communication
- **Trace context propagation**: `TracedEvent<T>` wrapper for cross-channel tracing
- **State persistence**: Alert state saved on shutdown
