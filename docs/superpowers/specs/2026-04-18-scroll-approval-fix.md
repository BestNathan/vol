# Conversation Scroll and Approval Panel Fix Design

## Goal

Fix two bugs: (1) conversation panel scroll broken due to Wrap changing visual line count, (2) approval panel not restoring after approval because state is not cleared.

## Root Cause

### Bug 1: Scroll
`Paragraph::wrap(Wrap { trim: false })` wraps long lines into multiple visual lines, but `scroll` offset is calculated as `lines.len()` which only counts logical lines. The offset is too small compared to actual visual line count.

### Bug 2: Approval panel stuck
`respond_approval()` sets response and notifies, but never clears `tool_name`/`reason`/`arguments` in `ApprovalState`. `has_pending_approval()` checks `tool_name` which remains `Some(...)`, so the panel stays visible.

## Fix

### 1. Pre-wrap conversation lines + remove Paragraph wrap

Replace `.wrap(Wrap { trim: false })` on the Paragraph with a `wrap_line(line, width)` helper in `build_conversation_lines()`. This pre-wraps long text into multiple `Line` entries so `lines.len()` = visual lines.

Remove `.wrap()` from the Paragraph in `render_conversation`.

### 2. Clear approval state in respond_approval

After setting the response, also call `state.approval_state.clear()` in `respond_approval()` to clear `tool_name`/`reason`/`arguments`.

### 3. Approval panel title

Change approval panel border title from "Input" to "Approval".

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-tui/src/ui/conversation.rs` | Add `wrap_line()` helper, remove `Wrap` from Paragraph |
| `crates/vol-llm-tui/src/main.rs` | Add `clear()` call to `respond_approval()` |
| `crates/vol-llm-tui/src/ui/input_area.rs` | Change approval panel title to "Approval" |
