---
type: entity
category: product
tags: [database, time-series, market-data]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# TDengine

**Category:** Time-series database
**Related:** [[tool-registry]], [[vol-llm-tool-crate]]

## Overview

TDengine is a time-series database used to store market data (prices, volatility, options) that the ReAct Agent queries through its tools.

## Key Facts
- Server at 192.168.2.106:6041 [[react-agent-docs]]
- Database: `deribit` [[react-agent-docs]]
- Tables used by agent tools: `deribit_index_price`, `deribit_volatility_index`, `deribit_options`, `deribit_rv` [[react-agent-docs]]
- Queried via REST API by tools like `market_data` and `alert_history` [[react-agent-docs]]

## Timeline
- **2026-04**: Integrated as data source for agent tools
