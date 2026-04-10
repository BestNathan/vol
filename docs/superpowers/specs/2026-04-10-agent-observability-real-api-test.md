# Agent Observability Real API Test

**Date:** 2026-04-10  
**Author:** Claude Code  
**Status:** Implemented

---

## Overview

Created an integration test example (`agent_observability_test.rs`) that demonstrates the observability plugin with:
- Real Anthropic/DashScope LLM API calls (qwen3.5-plus model)
- Real TDengine-based tools for market data queries
- JSONL file logging for all agent events

## Files Created

### Example File
- **Path:** `crates/vol-llm-agent/examples/agent_observability_test.rs`
- **Purpose:** Integration test with real API and tools
- **Run command:** `cargo run --example agent_observability_test`

## Requirements

### Environment Variables
```bash
export ANTHROPIC_AUTH_TOKEN=your_token_here
```

### TDengine Connection
Default configuration (connects to local TDengine server):
- Host: localhost
- Port: 6041
- Database: deribit

## Features Demonstrated

1. **Real LLM Integration**
   - Anthropic provider via DashScope endpoint
   - qwen3.5-plus model for Chinese language support
   - Streaming responses with tool calling

2. **Real Tool Integration**
   - `volatility_index`: Query deribit_volatility_index table
   - `index_price`: Query price data
   - `options`: Query options chain data
   - `rv`: Query realized volatility

3. **Observability Logging**
   - Session logs: `logs/agents/market_analyst_agent/sessions/session_<id>_<date>.jsonl`
   - Run logs: `logs/agents/market_analyst_agent/runs/run_<id>.jsonl`
   - JSONL format for structured, append-only logging

## Log Format

Each log entry is a JSON object with:
```json
{
  "timestamp": "2026-04-10T12:00:00Z",
  "run_id": "run_abc123",
  "agent_id": "market_analyst_agent",
  "event": "ToolCallComplete",
  "data": {
    "tool_name": "volatility_index",
    "result": "Index: btc_usd | Volatility: 85.2%..."
  }
}
```

## Usage

```bash
# Set API token
export ANTHROPIC_AUTH_TOKEN=sk-xxxxxxxx

# Run the test
cargo run --example agent_observability_test

# View logs
cat logs/agents/market_analyst_agent/runs/*.jsonl | jq .
```

## Expected Output

1. Console output showing agent execution events
2. JSONL log files created in agent-specific directories
3. Real market data queried from TDengine
4. Real LLM responses generated via DashScope

## Testing Scenarios

### Scenario 1: Basic Market Query
```
Query: "请查询 BTC 当前的波动率水平"
Expected: volatility_index tool called, logs created
```

### Scenario 2: Multi-Tool Analysis
```
Query: "请查询 BTC 当前的波动率水平和 ETH 的价格，并分析当前市场状况"
Expected: Multiple tool calls (volatility_index, index_price), final analysis
```

### Scenario 3: Alert Scenario
```
Query: "当前波动率是否异常？是否需要发送警报？"
Expected: Tool calls, analysis, potential alert recommendation
```

## Verification Checklist

- [ ] Example compiles without errors
- [ ] ANTHROPIC_AUTH_TOKEN environment variable check works
- [ ] Anthropic provider initializes successfully
- [ ] TDengine tools connect and query data
- [ ] Observability plugin creates log directories
- [ ] Session logs are written in JSONL format
- [ ] Run logs are written in JSONL format
- [ ] Console output shows all agent events
- [ ] Log files contain valid JSON entries

## Related Documentation

- [Observability Plugin Design](./2026-04-10-agent-observability-plugin-design.md)
- [Observability Implementation Plan](../plans/2026-04-10-agent-observability-implementation.md)
- [HITL Example](../../crates/vol-llm-agent/examples/agent_cli_approval.rs)

## Future Enhancements

1. Add configuration file support for TDengine connection
2. Add log viewing utility (jq filters, pretty printer)
3. Add log upload to Feishu for team visibility
4. Add metrics aggregation (token counts, tool call frequency)
