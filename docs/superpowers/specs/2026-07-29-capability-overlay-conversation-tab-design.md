# Capability Overlay — Move to Conversation Tab

**Date:** 2026-07-29
**Status:** draft

## Context

Capability overlay (dynamic tool/skill/MCP adjustment) is currently implemented as sections inside the Tools, Skills, and MCP panels. Users need to switch tabs away from the conversation to adjust capabilities. The overlay should be accessible directly from the Conversation tab, near the input area, where users spend most of their time.

## Design

### Conversation tab — capability summary row

A compact summary row between the conversation messages and the input box:

```
🛠 14 tools · 2 skills · 1 MCP  [✎]
```

- Shows effective counts (AgentDef defaults + overlay changes merged)
- Pure numbers, no individual tool names, no color coding
- `[✎]` button opens the dropdown panel

### Dropdown panel

Triggered by clicking `[✎]` or the summary row. Renders as a floating dropdown positioned near the summary row.

```
┌─ Capabilities ──────────────────────────┐
│ Tools                                   │
│ ☑ bash                                  │
│ ☑ read                                  │
│ ☑ +grep                                 │
│ ☐ write                                 │
│                                          │
│ Skills                                  │
│ ☐ code-review                           │
│                                          │
│ MCP Servers                             │
│ ☑ +docs-rs-mcp                          │
│                                          │
│ [Apply] [Reset to default]              │
└──────────────────────────────────────────┘
```

- Three groups (Tools / Skills / MCP) separated by dividers
- Items from AgentDef defaults shown normally
- Items added via overlay shown with `+` prefix (for visual distinction)
- Checkbox toggle sets local dirty state
- Apply → calls `agent.update_capabilities`, closes panel, updates summary row
- Reset → restores AgentDef defaults, sets dirty
- Click outside → closes without applying (local state discarded)

### Data flow

- `use_effect` watches selected agent, auto-loads capabilities via `agent.get_capabilities`
- Summary row counts: `effective_tools.len()` / `effective_skills.len()` / `effective_mcp_servers.len()`
- Overlay stored per `(agent_id, session_id)` — independent across agents
- Backend protocol unchanged: `agent.get_capabilities` / `agent.update_capabilities`

### Removal from existing panels

- Remove the Capability Overlay section from Tools, Skills, and MCP panels
- Keep the backend handler and protocol intact

## Files changed

| File | Change |
|------|--------|
| `crates/vol-llm-ui/src/web/components/conversation.rs` or conversation panel | Add capability summary row + dropdown component |
| `crates/vol-llm-ui/src/web/components/tools_panel.rs` | Remove Capability Overlay section |
| `crates/vol-llm-ui/src/web/components/skills.rs` | Remove Capability Overlay section |
| `crates/vol-llm-ui/src/web/components/mcp_panel.rs` | Remove Capability Overlay section |

## Verification

- Select agent → summary row shows `🛠 N tools · N skills · N MCPs`
- Click `[✎]` → dropdown opens with correct checkbox state
- Toggle items → Apply → summary counts update
- Reset → restores AgentDef defaults
- Switch agent → summary row updates for new agent
- Click outside dropdown → closes, changes discarded
