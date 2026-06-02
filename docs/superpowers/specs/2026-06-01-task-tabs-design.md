# Task Tabs Design

## Overview

Add two new task views: a global "Tasks" tab at the top level showing all tasks grouped by status, and a "Tasks" sub-tab in the agent panel showing tasks claimed by the selected agent. Read-only — no create/update/claim operations from the UI.

## Backend

### Protocol (`agent_server_protocol.rs`)

New `TaskOperation` enum (List, Get) and `TaskPayload` enum with list/get variants and their result types. Added to the existing `Operation` enum as `Operation::Task(TaskOperation)`.

### TaskHandler (`domain/task.rs`)

New `DomainHandler` implementation registering `task.list` and `task.get`. Holds `Arc<dyn TaskStore>` obtained from `runtime.task_store`.

- `task.list { status, assignee }` — calls `store.list(status)`, filters by assignee if provided, serializes tasks as JSON array
- `task.get { task_id }` — calls `store.get(&task_id)`, returns task JSON

### ServerCore

Registers `TaskHandler` in `build()` and `for_test()` using `self.runtime.task_store.clone()`.

## Client (`web/client.rs`)

New `TaskEntry` struct mirroring backend Task model fields. Two new RPC methods:

- `task_list(status, assignee, cb)` — returns `Vec<TaskEntry>`
- `task_get(task_id, cb)` — returns `TaskEntry`

## UI

### State (`state/mod.rs`)

New `TaskState` struct: tasks list, loading, error, status_filter, selected_task.

Enum changes:
- `ActiveTab` gains `Tasks` variant
- `AgentSubTab` gains `Tasks` variant

### TasksPanel Component (new)

Shared panel used by both global and agent-level views. Props: initial `assignee` filter (None for global, Some(agent_name) for agent panel).

Layout:
- Optional status filter bar: All / Pending / Running / Completed buttons
- Task list: each row shows task id (#t1), status badge (colored), subject, assignee, created_at
- Click row → expand inline detail panel showing description, dependencies, blocks, timestamps
- Auto-load on mount; retry on WS reconnect

### Top-Level Tasks Tab

`TabBar` gains a "Tasks" button as the first tab. `TabContent` routes `ActiveTab::Tasks` to `TasksPanel { assignee: None }`.

### Agent Panel Tasks Sub-Tab

`AgentsPanel` sub-tab bar gains a "Tasks" button (after Context). Routes `AgentSubTab::Tasks` to `TasksPanel { assignee: Some(selected_agent.name) }`.
