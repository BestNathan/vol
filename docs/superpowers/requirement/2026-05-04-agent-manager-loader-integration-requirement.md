# Requirements: Agent Manager Integration with Agent Loader

## Background

`vol-agent-manager` currently manages agents that are runtime processes connecting via WebSocket. Agent definitions (`.agents/agents/*.md` frontmatter files) are discovered by `AgentLoader` in `vol-llm-agent` but not used by the manager. We need a unified WS gateway that discovers agent definitions and dynamically routes to agent instances.

## Goals

1. **File-based agent discovery** — The manager integrates `AgentLoader` to discover agent definitions from `.agents/agents/` directories on startup.
2. **Path-based WS routing** — Clients connect to specific agent instances via URL path: `/ws/agents/:agent_type/session/:session_id`.
3. **Instance lifecycle via session** — Each `(agent_type, session_id)` uniquely identifies a running agent instance. Session is the instance identity.
4. **On-demand instantiation** — If `agent_type + session_id` has no existing instance, the manager automatically creates a new session and agent.
5. **Sub-task forking** — Agents can fork sub-tasks by creating new sessions with `parent_session_id`. The `agent_type` for sub-tasks is inferred from the `agent_path`.
6. **Multiple concurrent connections** — Multiple WS clients can connect to the same `agent_type + session_id` simultaneously.

## Non-Goals

- Agent-to-agent direct communication (managed via task dispatch only).
- Persistent state storage beyond session JSONL (no database).
- Multi-instance HA or state sync.
- Dashboard UI or role-based access control.

## Scope

### Included

- Add `vol-llm-agent` as a dependency to `vol-agent-manager`.
- Integrate `AgentLoader` into manager startup: discover agent definitions, populate available types.
- New WS route: `/ws/agents/:agent_type/session/:session_id`.
- Instance registry tracking `(agent_type, session_id) → AgentInstance`.
- On-demand agent instantiation from file definitions.
- Parent-child session tracking via `parent_session_id`.
- REST API: list available agent definitions, list running instances.
- `FileSessionEntryStore` enhancement: support `agent_type` subdirectory — store at `{entry_dir}/{agent_type}/{session_id}.jsonl`. Backward compatible: when `agent_type` is None/empty, use original `{entry_dir}/{session_id}.jsonl`.

### Excluded

- Changing the existing `/ws` endpoint behavior (backward compatible).
- Modifying `AgentLoader` discovery logic (already updated to use md-frontmatter).
- Changes to `vol-llm-agent` ReAct agent core.

## Constraints

- Agent instances are in-memory only — no cross-process persistence.
- Session persistence uses existing JSONL format (`FileSessionEntryStore`).
- Agent definitions are static; runtime instances are dynamic.
- WS protocol uses existing `WsMessage` envelope where possible.

## Success Criteria

1. Manager discovers agent definitions from `.agents/agents/` on startup.
2. `GET /ws/agents/:type/session/:sid` connects and spawns an agent instance if one does not exist.
3. Second connection to same `agent_type + session_id` routes to the existing instance.
4. `GET /api/v1/agent-types` returns discovered agent definitions.
5. `GET /api/v1/agent-instances` returns running instances with session metadata.
6. Sub-task forking creates child sessions with `parent_session_id` link.
7. All existing tests pass (backward compatible).
8. Sessions are stored under `{entry_dir}/{agent_type}/{session_id}.jsonl` when agent_type is provided.
9. Existing sessions without agent_type continue to work (no path change).

## Edge Cases

| Scenario | Behavior |
|---|---|
| `agent_type` not found in definitions | Return WS close with 404 reason, reject connection |
| `agent_type + session_id` no existing instance | Create new session + agent automatically |
| Multiple concurrent WS to same instance | All connections share the same agent; messages broadcast or routed appropriately |
| WS client disconnects | Agent instance continues running (session lifecycle is independent) |
| Sub-task fork without explicit agent_type | Infer from `agent_path` (parent agent's type hierarchy) |
| Agent crashes/panics | Instance removed from registry; next connection creates fresh instance |
| Duplicate session_id across different agent_types | Allowed — session_id is scoped per agent_type |

## Open Questions

1. **Agent lifecycle** — Follow session lifecycle. Instances are destroyed when the associated session is deleted.
2. **Concurrent client routing** — Broadcast agent output to all connected WS clients on the same instance.
