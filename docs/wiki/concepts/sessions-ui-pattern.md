---
type: concept
category: pattern
tags: [dioxus, web, sessions, ui, component]
created: 2026-05-11
updated: 2026-05-11 (task-6-sessions-tab-wiring)
source_count: 1
---

# Sessions UI Pattern

**Category:** Web frontend component pattern for session browsing
**Related:** [[dioxus-web-pattern]], [[dioxus-signal-pattern]], [[vol-llm-ui-crate]]

## Definition

A dedicated tab-based session browsing interface using `SessionsState` signal for local state management, `SessionsPanel` component for rendering, and tab routing via `ActiveTab::Sessions`.

## Key Points

- **Signal-based state**: `SessionsState` holds `sessions: Vec<SessionListEntry>`, `loading: bool`, `error: Option<String>` — created in `App()` via `use_signal` and provided via `use_context_provider` [[dioxus-signal-pattern]]
- **Component**: `SessionsPanel` reads the signal from context, displays loading/empty/error states, renders session items with id, message count, and age
- **Tab integration**: Sessions is a first-class tab in TabBar, positioned between Conversation and Tools
- **CSS classes**: `.sessions-panel`, `.sessions-panel-header`, `.sessions-panel-loading`, `.sessions-panel-empty`, `.sessions-panel-error`, `.session-item`, `.session-item-id`, `.session-item-count`, `.session-item-age`
- **Replaces SessionDialog**: The original modal-based `SessionDialog` was removed from web UI render; session browsing is now tab-based

## How It Works

```
App()
├── sessions_signal = use_signal(|| SessionsState::new())
├── use_context_provider(|| sessions_signal)
└── TabContent
    └── ActiveTab::Sessions => SessionsPanel {}

SessionsPanel
├── reads Signal<SessionsState> from context
├── loading → .sessions-panel-loading
├── error → .sessions-panel-error
├── empty → .sessions-panel-empty
└── has sessions → .session-item for each
    ├── .session-item-id (session ID, monospace)
    ├── .session-item-count (message count)
    └── .session-item-age (relative timestamp)
```

## Checkpoint Rendering

`EntryCheckpoint` entries in the conversation view render with the `msg-checkpoint` CSS class — a yellow-tinted bar with left border indicating archived message boundaries. This visual marker appears when sessions use checkpoint entries to separate conversation segments.

## Examples / Applications
- Sessions tab in web frontend — lists available sessions with metadata, allows browsing session history
- Conversation checkpoint entries — visual separator showing where previous messages were archived

## Related Concepts
- [[dioxus-web-pattern]]: Parent pattern describing overall web architecture
- [[dioxus-signal-pattern]]: Signal-based state management used by SessionsPanel
- [[vol-llm-ui-crate]]: Crate containing state types and components
- [[session-as-ssot]]: Session as single source of truth for messages
