---
type: concept
category: pattern
tags: [human-in-the-loop, approval, workflow]
created: 2026-05-04
updated: 2026-05-04
source_count: 2
---

# Human-in-the-Loop

**Category:** Approval workflow
**Related:** [[agent-plugin-system]], [[plugin-actions]], [[built-in-plugins]], [[agent-event-stream]], [[subagent-review-pattern]], [[clarifying-requirements-workflow]]

## Definition

A plugin that requires human approval before executing tool calls or continuing agent iterations.

## Key Points
- Configurable triggers: tool execution or iteration continuation [[react-agent-docs]]
- Supports specific tool targeting or all-tools approval [[react-agent-docs]]
- TUI frontend renders approval panel with A/R/S key handling [[tui-frontend-ratatui]]
- Web frontend renders ApprovalDialog modal component with approve/reject/stop buttons [[dioxus-web-pattern]]
- Timeout handling with configurable behavior (reject, stop, continue) [[react-agent-docs]]
- Two approval channels: CLI (terminal prompts) and HTTP (remote callbacks) [[react-agent-docs]]

## How It Works

The HITL plugin intercepts `ToolCallBegin` events and pauses execution until a human approves or rejects:

```rust
let config = HitlConfig {
    triggers: vec![ApprovalTrigger::ToolExecution { tools: None }], // all tools
    timeout_secs: 300,
    on_timeout: TimeoutBehavior::Stop,
    ..Default::default()
};
let channel = Arc::new(CliApprovalChannel); // or SimpleHttpApprovalChannel
let plugin = HitlPlugin::new(config, channel);
```

For HTTP approval, the plugin exposes an endpoint that receives approval/rejection decisions. The HTTP router can be created via `channel.create_router()`.

## Examples / Applications

- **Critical operations**: Require approval before executing trades or data modifications
- **Debugging**: Pause agent execution at each step to inspect reasoning
- **Cost control**: Approve expensive tool calls that consume many tokens

## Related Concepts
- [[agent-plugin-system]]: How the plugin integrates
- [[plugin-actions]]: Uses Abort for rejection, Continue for approval
- [[agent-event-stream]]: Intercepts ToolCallBegin events
