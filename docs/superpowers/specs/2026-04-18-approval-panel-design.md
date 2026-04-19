# Approval Panel Replace Input Box Design

## Goal

When a HITL approval is triggered, replace the entire input box area with an approval panel. After approval completes, restore the input box. Remove the existing floating approval banner from the conversation panel.

## Current State

- `approval_banner.rs` renders a 5-line floating banner overlay on the conversation panel
- `input_area.rs` shows approval hints in the bottom hint row but the textarea remains visible
- Two separate approval UI elements exist: banner (conversation panel) + hints (input area)

## Design

### Layout Behavior

When `state.approval_state.has_pending_approval()` is true:
- The entire input area (`chunks[2]`, 5 rows) renders the approval panel instead of the textarea
- No floating banner in conversation panel
- When approval completes, the input area returns to normal textarea rendering

### Approval Panel Content (balance mode)

```
┌───────────── Input ─────────────┐
│ ⚠ BashTool                     │
│ rm -rf /tmp/x...                │
│                                 │
│ [A] Approve  [R] Reject  [S] Stop│
└─────────────────────────────────┘
```

- Line 1: Tool name with warning icon style (yellow)
- Line 2: Command preview extracted from arguments JSON — parse `arguments.command` field, truncate to panel width minus indentation. If parsing fails or no command field, fall back to showing first 60 chars of raw arguments string.
- Line 3: Spacer
- Line 4: Action hints — A (green), R (red), S (red)

### Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-tui/src/ui/input_area.rs` | Add approval panel rendering as alternative to textarea |
| `crates/vol-llm-tui/src/ui/approval_banner.rs` | Delete — no longer needed |
| `crates/vol-llm-tui/src/ui/mod.rs` | Remove approval_banner export and usage |
| `crates/vol-llm-tui/src/ui/conversation.rs` | Remove `render_approval_banner` call |

### Data Flow

```
Agent detects dangerous tool → ApprovalState fields populated
  → has_pending_approval() returns true
    → render_input_area() renders approval panel instead of textarea
      → User presses A/R/S → respond_approval() sends response
        → ApprovalState fields cleared
          → has_pending_approval() returns false
            → render_input_area() renders textarea again
```

### Key handling (unchanged)

`handle_key` already checks `has_pending_approval()` first and intercepts A/R/S keys. No changes needed to `main.rs`.
