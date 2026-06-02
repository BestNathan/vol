# Requirements: Agent Teamwork Capability

## Background

The agent system currently operates in isolation — each agent handles user requests independently with no ability to collaborate. Agents cannot publish work items, assign tasks to other agents, or claim available work. This limits the system's ability to handle complex multi-agent workflows.

The goal is to add teamwork capabilities so agents can coordinate like a team: publish tasks to a shared pool, assign work to specific agents, claim available tasks, and execute them.

## Goals

1. **Shared task pool** — all agents within the same runtime can read and write to a single, shared `TaskStore`
2. **Publish tasks** — agents can create tasks with their own identity recorded as publisher, optionally specifying a target agent as assignee
3. **Claim tasks** — agents can atomically claim pending tasks (either assigned to them specifically, or from the open pool)
4. **Agent identity in tool context** — `ToolContext` carries the calling agent's full `AgentDef`, so task tools can record who did what
5. **AgentRuntime crate** — a new `vol-llm-runtime` crate that owns core runtime resources (LLM, ToolRegistry, TaskStore, agent router, definitions, status), separating runtime concerns from transport concerns

## Non-Goals

- **No task approval workflow** — publishing a task makes it immediately claimable; no manager review gate
- **No inter-agent direct messaging** — agents communicate through the task pool, not via DM/chat
- **No cross-process task pool** — single runtime instance only; no distributed task queue
- **No task scheduling/priority queue** — agents claim tasks voluntarily; no automated dispatching or priority ordering
- **No agent capability-to-task matching enforcement** — any agent can claim any task; filtering is up to the LLM's judgment

## Scope

### Included

| Item | Description |
|------|-------------|
| `vol-llm-runtime` crate | New independent crate holding `AgentRuntime` with all core runtime structures |
| `Task` model extension | Add `publisher: Option<String>` and `assignee: Option<String>` to `Task`; leverage existing `dependencies`/`blocks` for task readiness checks |
| `ToolContext.agent_def` | Add `agent_def: Option<AgentDef>` to `ToolContext` |
| `task_create` extension | Extend `task_create` to accept optional `assignee` param; publisher auto-populated from ToolContext.agent_def |
| `task_claim` tool | New tool: atomically claims a pending task for the calling agent |
| `task_list` enhancement | Filter by assignee (mine / unassigned / specific) and status |
| `AgentServerCore` refactor | Uses `AgentRuntime` internally; registers teamwork tools into shared registry |
| Web backend integration | `vol-agent-manager` uses `AgentRuntime`; per-agent tool creation uses shared registry |

### Excluded

- New agent communication protocol messages for task events (future: push notification of new tasks)
- Task deadline enforcement
- Task templates or recurring tasks
- Web UI for task board (separate feature)

## Constraints

- Use existing `vol-llm-task` crate's `Task`, `TaskStatus`, `TaskStore`, `TaskId` abstractions — extend, don't replace
- Use existing `vol-llm-tool` crate's `ToolContext`, `ExecutableTool`, `ToolRegistry` — extend, don't replace
- `AgentRuntime` must be usable by both `AgentServerCore` (agent-channel) and `vol-agent-manager` (web backend)
- Backward compatibility: existing `task_create` tool behavior preserved; new fields are additive (Option types)
- Must compile with workspace Rust edition 2021

## Success Criteria

1. `vol-llm-runtime` crate exists, compiles, and is depended on by `vol-llm-agent-channel`
2. `AgentRuntime` struct owns: LLM client, ToolRegistry, TaskStore, AgentRouter, agent defs, agent status, MCP manager, SkillLoader
3. `AgentServerCore` delegates to `AgentRuntime` for all runtime concerns
4. `ToolContext` carries `agent_def: Option<AgentDef>`, populated automatically when agents call tools
5. `Task` model has `publisher: Option<String>` and `assignee: Option<String>` fields
6. `task_create` extended: auto-populates `publisher` from ToolContext.agent_def, accepts optional `assignee` param
7. `task_claim` tool atomically claims a task (status Pending→Running, assignee set), returns task content for execution; fails if task is already claimed
8. `task_list` supports filtering by assignee (`assignee=<agent_type>` or `unassigned`)
9. Agent A can publish a task, Agent B can discover it via `task_list`, claim it via `task_claim`, and execute it
10. All existing tests continue to pass

## Edge Cases

| Scenario | Expected Behavior |
|----------|-------------------|
| Two agents claim same task concurrently | Atomic claim — first wins, second gets error "task already claimed" |
| Task published with non-existent assignee | Accept the task (don't validate agent existence at publish time); it stays in pool as assigned but unclaimed |
| Agent disconnects while executing claimed task | Task stays Running; future: health checker resets to Pending |
| Pending task never claimed | Stays Pending indefinitely; visible in list, no auto-timeout |
| Agent fails execution of claimed task | Task status → Failed; publisher or any agent can publish a new follow-up task |
| Agent tries to claim already-Running task | `task_claim` returns error "task is not in Pending status" |
| Agent claims task with uncompleted dependencies | `task_claim` rejects — uses existing `Task.dependencies` and `TaskStore.get_ready_tasks()` to enforce readiness |
| Agent publishes task and also wants to execute it | Can call `task_claim` on their own published task (self-claim) |

## Open Questions

- **Task event notifications** — currently agents discover tasks by polling `task_list`. Should `AgentRuntime` also push events to connected agents when a task is published/assigned to them? Deferred to brainstorming phase.
- **Stale task recovery** — if an agent crashes mid-execution, task stays Running forever. Should a watchdog reset stale tasks? Deferred.
