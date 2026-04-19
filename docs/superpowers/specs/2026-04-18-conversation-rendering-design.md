# Conversation Rendering Optimization Design

## Goal

Fix four UI rendering issues in the conversation panel: content overflow, delayed thinking/content display, duplicate agent responses, and scroll control.

## Root Cause

Current data flow buffers all deltas and only pushes entries on `*Complete` events:

```
ThinkingDelta × N  → buffer → ThinkingComplete → 1 entry (all at once)
ContentDelta × N   → buffer → ContentComplete → AgentAnswer
AgentComplete      → extracts response.content → AgentAnswer (DUPLICATE)
```

Paragraph has no wrap, scroll uses entry count, and scroll keys are PageUp/PageDown only.

## Changes

### 1. Real-time Thinking — in-place last entry update

- `ThinkingStart` → push `Thinking { content: "" }` to conversation
- `ThinkingDelta` → find last `Thinking` entry, `content.push_str(delta)`
- `ThinkingComplete` → no-op (content already streamed)

### 2. Real-time Content — same pattern

- `ContentStart` → push `ContentStreaming { content: "" }` to conversation
- `ContentDelta` → find last `ContentStreaming` entry, `content.push_str(delta)`
- `ContentComplete` → mutate last `ContentStreaming` entry to `AgentAnswer` (single source)

### 3. Eliminate Duplicate Content

- `AgentComplete` must NOT extract `response.content` and push `AgentAnswer`
- Streamed content from `ContentStreaming` → `AgentAnswer` conversion is the sole source
- If `ContentComplete` was never received (edge case), `flush_content` handles it

### 4. Text Wrapping

- Conversation entries with long text (`AgentAnswer`, `Thinking`, `ContentStreaming`)
  use `Paragraph` with `Wrap { trim: false }` instead of pre-split `Line`s
- Short entries (ToolCall, RunSummary, Error) keep using `Line` arrays

### 5. Scroll with ⬆️⬇️

- `⬆️` → `conversation_scroll = scroll.saturating_sub(1)`
- `⬇️` → `conversation_scroll += 1` (capped at `total_lines - visible_height`)
- PageUp/PageDown → 10-line step
- Disable auto-scroll when user manually scrolls (clear `conversation_auto_scroll`)

## Files Changed

| File | Changes |
|------|---------|
| `crates/vol-llm-tui/src/app.rs` | Replace `ThinkingComplete` with `Thinking { content }`, add `ContentStreaming { content }` |
| `crates/vol-llm-tui/src/render.rs` | Update ThinkingDelta/ContentDelta to mutate last entry; remove duplicate in AgentComplete |
| `crates/vol-llm-tui/src/ui/conversation.rs` | Render `Thinking`/`ContentStreaming` with `Paragraph::wrap()`, remove old variants |
| `crates/vol-llm-tui/src/main.rs` | ⬆️⬇️ scroll handling, PageUp/PageDown → 10-line step |

## Data Flow After

```
ThinkingStart    → push Thinking { "" }           ← UI shows "Thinking..."
ThinkingDelta    → append to last Thinking        ← UI updates incrementally
ThinkingComplete → no-op
ContentStart     → push ContentStreaming { "" }   ← UI shows empty content area
ContentDelta     → append to last ContentStreaming ← UI updates incrementally
ContentComplete  → finalize (mark complete)
AgentComplete    → summary only, no content       ← no duplicate
```
