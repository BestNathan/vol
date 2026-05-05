---
type: concept
category: pattern
tags: [react-agent, agent-pattern, llm]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# ReAct Pattern

**Category:** Agent execution pattern
**Related:** [[agent-plugin-system]], [[agent-event-stream]], [[tool-registry]]

## Definition

ReAct (Reason + Act) is a paradigm where an LLM alternates between reasoning about a problem and taking actions (calling tools) to gather information, building up a chain of reasoning and observations before producing a final response.

## Key Points
- Each cycle: **Reason** (LLM decides next step) → **Act** (execute tool) → **Observe** (collect result) → repeat [[react-agent-docs]]
- Maximum iterations configurable (default 5) to prevent infinite loops [[react-agent-docs]]
- Agent can produce a final response at any point instead of calling a tool [[react-agent-docs]]
- Supports both single-turn and multi-turn conversations [[react-agent-docs]]

## How It Works

1. User input is combined with system prompt into initial message
2. LLM is called with tool definitions — it either requests tool calls or produces final text
3. If tool calls: execute them via [[tool-registry]], append results to conversation
4. Loop back to step 2 until max iterations or final response
5. Final response is returned to user

The pattern is implemented in `ReActAgent` with a fluent builder (`[[agent-builder-pattern]]`) for configuration.

## Examples / Applications

- **Market data queries**: "What is the current BTC price?" → calls `market_data` tool → returns price
- **Volatility analysis**: "Show me ETH volatility" → calls `alert_history` tool → returns analysis
- **Compound questions**: "What is BTC price and how does it compare to ETH?" → multiple tool calls
- **Greetings**: "Hello" → no tool needed → direct response

## Related Concepts
- [[agent-plugin-system]]: Plugins intercept events in the ReAct loop
- [[agent-event-stream]]: Events emitted during each ReAct cycle
- [[agent-builder-pattern]]: How ReActAgent is configured and built
- [[tool-registry]]: Tools available for the Act phase
- [[session-as-ssot]]: How messages are managed across ReAct cycles
- [[run-context]]: Run state tracked during ReAct execution
- [[context-builder]]: How prompt context is assembled for each LLM call
