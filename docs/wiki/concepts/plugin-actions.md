---
type: concept
category: pattern
tags: [plugin, action-types, control-flow]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Plugin Actions

**Category:** Control flow pattern
**Related:** [[agent-plugin-system]], [[built-in-plugins]]

## Definition

The return types that plugins use to control agent execution flow: Continue, Continue(None), ShortCircuit, Skip, and Abort.

## Key Points
- `Continue(event)`: Pass event to next plugin in chain [[react-agent-docs]]
- `Continue(None)`: Drop event, get next event from stream [[react-agent-docs]]
- `ShortCircuit(response)`: Return response immediately, skip remaining plugins and agent logic [[react-agent-docs]]
- `Skip`: Skip this event entirely (no output) [[react-agent-docs]]
- `Abort(error)`: Terminate agent execution with error [[react-agent-docs]]

## How It Works

Each plugin's `intercept` method returns a `PluginAction<T>` that determines what happens to the current event and whether agent execution continues. The action is evaluated in priority order — once a plugin returns `ShortCircuit` or `Abort`, subsequent plugins in the chain do not see the event.

`ShortCircuit` is used by [[semantic-caching]] to return cached responses without hitting the LLM. `Abort` is used by [[rate-limiting]] when the concurrency limit is exceeded, and by error handlers when a critical failure occurs.

## Examples / Applications

- **Caching plugin**: Returns `ShortCircuit(cached_response)` on cache hit
- **Rate limiter**: Returns `Abort(rate_limit_exceeded)` when semaphore is full
- **Logging plugin**: Returns `Continue(event)` to pass through while recording
- **Filter plugin**: Returns `Skip` for events that should not reach the output

## Related Concepts
- [[agent-plugin-system]]: Where plugin actions are defined and executed
- [[built-in-plugins]]: How each built plugin uses these actions
- [[agent-event-stream]]: What the actions operate on
