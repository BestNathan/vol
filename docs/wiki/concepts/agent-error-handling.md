---
type: concept
category: pattern
tags: [error-handling, agent, retry, resilience]
created: 2026-05-14
updated: 2026-05-14
source_count: 1
---

# Agent Error Handling

**Category:** Error management and retry strategy

**Related:** [[agent-tool-design]], [[agent-builder-pattern]], [[retry-with-backoff]]

## Definition

Hierarchical error types for the ReAct agent system using `thiserror` derives, with a retry strategy that distinguishes retryable from non-retryable errors.

## Key Points

- Error hierarchy: `AgentError` with variants for `LlmExecution`, `ToolExecution`, `MaxIterationsReached`, `InvalidToolResponse`, and `Context`
- Retryable errors: `Network`, `RateLimit`, and `5xx` server errors trigger exponential backoff retry
- Non-retryable errors: `InvalidToolResponse`, `MaxIterationsReached`, and client errors fail immediately
- Error types derive `thiserror::Error` for automatic `Display` and `Debug` implementations
- Streaming agent extension includes `AgentStreamEvent` for real-time error reporting

## Retry Strategy

The retry strategy applies exponential backoff with configurable max retries:
- Initial delay: 1 second
- Backoff multiplier: 2x per retry
- Maximum retries: 3 (configurable)
- Only retryable error variants trigger retry logic

## Related Concepts
- [[agent-tool-design]]: Original design document specifying error hierarchy
- [[retry-with-backoff]]: Retry implementation pattern used by agent error handling
- [[agent-builder-pattern]]: Builder pattern for configuring retry parameters
