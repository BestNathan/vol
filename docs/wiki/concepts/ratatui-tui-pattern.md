---
type: concept
category: pattern
tags: [ratatui, tui, rendering, layout, widget]
created: 2026-05-08
updated: 2026-05-08
source_count: 1
---

# Ratatui TUI Rendering Patterns

**Category:** UI rendering pattern for terminal applications

**Related:** [[vol-llm-ui-crate]], [[tui-frontend-ratatui]]

## Definition

Layout and widget composition patterns used in the vol-llm-ui TUI frontend, built on ratatui 0.30 with crossterm 0.29 backend.

## Key Points
- Uses `Layout::default().direction().constraints()` for all spatial partitioning
- Status bar (1 row) + main area split: tools panel (30%) | right panel (70%)
- Right panel split: tab bar (1 row) | tab content (Min) | input area (5 rows)
- Dialog overlays render on top of the full area with `Clear` widget to prevent bleed-through
- All rendering functions take `&mut Frame`, `Rect`, and `&UiState` — no mutable state mutation during render

## How It Works

The render tree:
```
Frame
├── Status Bar (top 1 row, dark gray background)
└── Main Area
    ├── Tools Panel (30% left, tool call list with status colors)
    └── Right Panel (70%)
        ├── Tab Bar (Conversation | Workspace | Skills | Logs)
        ├── Tab Content (varies by active tab)
        └── Input Area (5 rows, shows approval panel or text hints)
    └── Session Dialog (overlay, centered, conditional)
```

Each tab content renderer:
- **Conversation**: Builds styled `Line` vectors from `ConversationEntry` enum with word wrapping, auto-scroll support
- **Workspace**: Displays directory tree with `[DIR]`/`[FILE]` prefixes and modification indicators
- **Skills**: Columnar layout with name, version, scope, description fields
- **Logs**: Two-level view — run list or log entries per selected run

Widget conventions:
- `Block::default().borders(Borders::ALL).title(...)` for all bordered sections
- Color coding: Green=success, Red=error, Yellow=warning/thinking, Blue=tools, Cyan=user input, DarkGray=secondary text
- ASCII characters for status indicators (`OK`, `ERR`, `...`, `SKIP`) instead of unicode symbols

## Examples / Applications
- `render_conversation`: Uses `build_conversation_lines` to convert `ConversationEntry` variants into styled `Line<'static>` with word wrapping
- `render_tools_panel`: Uses `List` and `ListItem` for scrollable tool call display
- `render_session_dialog`: Uses `ratatui::widgets::Clear` to prevent background bleed-through in overlay

## Related Concepts
- [[ui-event-loop-pattern]]: The event loop that drives rendering at 30fps
- [[human-in-the-loop]]: Approval panel rendering within the input area
