# QA Report — vol-llm-ui

**Date:** 2026-08-05
**URL:** http://localhost:5173
**Branch:** main
**Mode:** Standard (connected + disconnected states)
**Framework:** React 18 + Vite + Tailwind
**Backend:** vol-agent-server (data-plane, port 3001)

---

## Health Score

| Category | Score | Weight | Notes |
|----------|-------|--------|-------|
| Console (connected) | 100 | 15% | Zero errors when backend is up |
| Console (disconnected) | 10 | — | Spam: 183 warnings in ~5min |
| Links | 100 | 10% | No broken links |
| Visual | 100 | 10% | Layout renders correctly |
| Functional | 75 | 20% | Core works; 5 tabs blocked without control plane |
| UX | 65 | 15% | Mobile debug panel blocks UI; no Escape dismiss |
| Performance | 80 | 10% | Duplicate JSON-RPC calls |
| Content | 95 | 5% | All placeholder texts correct |
| Accessibility | 85 | 15% | Buttons have labels; tab state visible |
| **Weighted Score** | **80.25** | | |

---

## Issues Found

### ISSUE-001 (MEDIUM): No local node in data-plane-only mode

**Severity:** Medium | **Category:** Functional
**Found in:** Agents, Tasks, Skills, MCP, Logs tabs

When connected to a standalone data-plane server (control_plane=false), 5 out of 7 tabs show "Select a node to view X" because no nodes are returned by `control.node_list`. Only Tools and Workspace work without a node.

**Expected:** Either auto-register a local node for the direct connection, or provide a fallback UI.

**Repro:**
1. Start vol-agent-server in data-plane-only mode
2. Open http://localhost:5173
3. Click Tasks, Agents, Skills, MCP, or Logs tab
4. See: "Select a node to view X"

### ISSUE-002 (MEDIUM): Debug panel blocks all interaction in mobile viewport

**Severity:** Medium | **Category:** UX
**Found in:** DebugPanel component

At viewport widths where the debug panel renders as a full-screen modal (class: `fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4`), it intercepts all pointer events on the page. Neither Escape key nor backdrop click dismisses it reliably.

**Repro:**
1. Set viewport to narrow width (~400px)
2. Click 🐛 to open debug panel
3. Try to click any tab button → blocked
4. Press Escape → panel stays open
5. Click backdrop → panel stays open

### ISSUE-003 (LOW): Duplicate `tool.list` calls

**Severity:** Low | **Category:** Performance
**Found in:** ToolsTab component

Each time the Tools tab is opened, two identical `tool.list` JSON-RPC calls are sent simultaneously (visible in debug panel at 00:03:02.423 and 00:03:18.475).

**Expected:** Single `tool.list` call per tab activation.

### ISSUE-004 (LOW): Escape key doesn't close debug panel

**Severity:** Low | **Category:** UX
**Found in:** DebugPanel component

Pressing the Escape key does not close the debug panel. This violates the standard modal/drawer UX pattern.

### ISSUE-005 (OBSERVATION): WebSocket reconnection generates excessive console noise

**Severity:** Observation | **Category:** Console
**Found in:** lib/jsonrpc-client.ts, lib/reconnect.ts

When the backend is down, the reconnection loop generates 183+ warnings in ~5 minutes. Each failed connection attempt rejects all pending calls (line 115), producing one warning per pending call. Combined with aggressive retry, this floods the console. The exponential backoff (3s→30s, 10 attempts max) is correct but the initial burst of rejections is noisy.

**Note:** This is expected behavior when backend is unreachable. Not a bug per se, but the console noise level should be reduced.

---

## Feature Checklist

### Connection & Protocol

| Feature | Status | Notes |
|---------|--------|-------|
| WebSocket connect | ✅ PASS | Connects to ws://localhost:3001/ws |
| JSON-RPC 2.0 protocol | ✅ PASS | Correct format: `{"jsonrpc":"2.0","id":N,"result":{...}}` |
| `system.connected` | ✅ PASS | Returns `server_type: "DataPlane"`, version, capabilities |
| `agent.subscribe` | ✅ PASS | Auto-sent on connect, response received |
| `tool.list` | ✅ PASS | Returns 39 tools with descriptions |
| `control.node_list` | ⚠️ NO RESPONSE | Expected in data-plane-only mode (no control plane) |
| Reconnect on disconnect | ✅ PASS | Exponential backoff 3s→30s, 10 attempts |
| Status bar: Connected | ✅ PASS | Shows "Connected" when WS is up |
| Status bar: Session | ✅ PASS | Shows "Session: web-sess" |
| Status bar: Run/Iter/Tools/Time | ✅ PASS | Shows 0/0/0/00:00 when idle |
| Status bar: Idle | ✅ PASS | Shows "Idle" when no active run |

### Top Navigation Tabs

| Tab | Clickable | Active State | Content | Notes |
|-----|-----------|-------------|---------|-------|
| Tasks | ✅ | ✅ `[active]` | "Select a node..." | Requires node |
| Agents | ✅ | ✅ `[active]` | "Select a node..." | Requires node |
| Tools | ✅ | ✅ `[active]` | 39 tools listed | ✅ Works without node |
| Workspace | ✅ | ✅ `[active]` | "Click a file..." | ✅ Works without node |
| Skills | ✅ | ✅ `[active]` | "Select a node..." | Requires node |
| MCP | ✅ | ✅ `[active]` | "Select a node..." | Requires node |
| Logs | ✅ | ✅ `[active]` | "Select a node..." | Requires node |

### Tools Tab (fully tested)

| Feature | Status | Notes |
|---------|--------|-------|
| Tool list display | ✅ PASS | 39 tools: bash, echo-tool, edit_file, glob, grep, read_file, skill, task, web_fetch, web_search, write_file + 23 MCP tools (docs-rs-http × 4, playwright × 27) |
| Tool descriptions | ✅ PASS | Each tool shows its description |
| Per-tool "Run" button | ✅ PRESENT | 39 Run buttons, one per tool |
| Refresh button | ✅ PRESENT | Visible in header |
| Empty call history | ✅ PASS | "No tool calls yet — click Run on a tool above" |
| System Tools count | ✅ PASS | Shows "(39)" in header |

### Debug Panel

| Feature | Status | Notes |
|---------|--------|-------|
| Toggle open | ✅ PASS | 🐛 button in status bar |
| Toggle close (× button) | ✅ PASS | Close button works |
| WS filter button | ✅ PASS | Toggles `[active]` state |
| Message list | ✅ PASS | Shows timestamp, direction (→/←), method |
| Message expand/collapse | ✅ PASS | Click to expand JSON payload |
| JSON payload display | ✅ PASS | Pretty-printed JSON-RPC payloads |
| Direction arrows | ✅ PASS | → for outgoing, ← for incoming |
| Message count | ✅ PASS | "10 messages" / "18 messages" |
| Recording status | ✅ PASS | "Recording since page load" |
| Backdrop dismiss | ❌ FAIL | Backdrop click doesn't close (ISSUE-002) |
| Escape dismiss | ❌ FAIL | Escape key doesn't close (ISSUE-004) |
| Mobile modal behavior | ❌ FAIL | Blocks all interaction (ISSUE-002) |

### File Explorer

| Feature | Status | Notes |
|---------|--------|-------|
| Desktop: persistent sidebar | ✅ PASS | Shows "Explorer" header |
| Desktop: empty state | ✅ PASS | "No node selected" |
| Mobile: "📂 Files" toggle | ✅ PASS | Button visible |
| Mobile: open drawer | ✅ PASS | Opens file explorer |
| Close (✕) button | ✅ PASS | Closes explorer |
| Refresh button (per-dir) | ⚠️ NOT TESTED | No directories visible without node |

### Tools Tab — Individual Tool "Run" Buttons

| Tool | Status | Notes |
|------|--------|-------|
| bash | ✅ PRESENT | Button visible |
| echo-tool | ✅ PRESENT | Button visible |
| edit_file | ✅ PRESENT | Button visible |
| glob | ✅ PRESENT | Button visible |
| grep | ✅ PRESENT | Button visible |
| read_file | ✅ PRESENT | Button visible |
| skill | ✅ PRESENT | Button visible |
| task | ✅ PRESENT | Button visible |
| web_fetch | ✅ PRESENT | Button visible |
| web_search | ✅ PRESENT | Button visible |
| write_file | ✅ PRESENT | Button visible |
| mcp__docs-rs-http_* (×4) | ✅ PRESENT | All 4 have Run buttons |
| mcp__playwright_* (×27) | ✅ PRESENT | All 27 have Run buttons |

### Responsive Design

| Feature | Status | Notes |
|---------|--------|-------|
| Desktop (1280×720) | ✅ PASS | Sidebar + tabs + content layout correct |
| Mobile | ⚠️ ISSUE | Debug panel blocks interaction |

### Backend Protocol

| Check | Status | Notes |
|-------|--------|-------|
| JSON-RPC init response | ✅ PASS | Correctly returns capabilities list |
| Server version | ✅ PASS | "0.1.0" |
| Server type | ✅ PASS | "DataPlane" |
| MCP servers connected | ⚠️ PARTIAL | docs-rs-http ✅, 钉钉文档 ✅, playwright ✅, docs-rs-mcp ❌ (DNS), Deribit ❌ (SSL), docs-rs ❌ (timeout) |
| WS path | ✅ PASS | /ws |
| Auto-subscribe | ✅ PASS | agent.subscribe sent after connect |
| Pending call reject on close | ✅ PASS | Calls rejected with -1 on disconnect |

---

## Protocol Log (sanitized)

```
00:00:00.000 → control.node_list
00:00:02.390 → system.connected
00:00:02.390 → agent.subscribe
00:00:02.391 ← <response> {"jsonrpc":"2.0","id":1,"result":{"server_type":"DataPlane","version":"0.1.0","capabilities":[...]}}
00:00:02.391 ← <response> (agent.subscribe result)
00:00:02.888 → system.connected (reconnect)
00:00:02.889 → agent.subscribe (reconnect)
00:00:02.889 ← <response>
00:00:02.930 ← <response>
00:00:02.930 ← <response>
00:03:02.423 → tool.list
00:03:02.423 → tool.list (DUPLICATE)
00:03:02.438 ← <response>
00:03:02.470 ← <response>
00:03:18.475 → tool.list
00:03:18.475 → tool.list (DUPLICATE)
00:03:18.491 ← <response>
00:03:18.526 ← <response>
```

---

## Summary

### What Works
- WebSocket connection establishes cleanly, no errors when backend is up
- JSON-RPC 2.0 protocol is correctly implemented
- `system.connected` returns proper server metadata
- `tool.list` returns all 39 tools (local + MCP)
- Status bar metrics (Connected, Session, Run, Iter, Tools, Time, Idle) all display correctly
- All 7 tab buttons click and show correct active state
- Debug panel captures and displays protocol messages with expandable JSON
- File explorer toggles open/close correctly
- Tools tab shows all tools with per-tool Run buttons
- Tool descriptions and metadata display correctly

### What Needs Attention
1. **Most tabs useless without control plane**: Agents, Tasks, Skills, MCP, Logs require node selection but no nodes exist in data-plane-only mode
2. **Debug panel mobile blocking**: At narrow viewports, the debug panel becomes a full-screen modal that can't be dismissed
3. **Duplicate tool.list calls**: Two identical calls sent every time Tools tab opens
4. **Escape key ignored**: Debug panel doesn't close on Escape
5. **Console noise on disconnect**: 183 warnings in 5 minutes when backend is down

### Top 3 Things to Fix
1. **Auto-register local node in data-plane mode** — unlocks 5 tabs for local dev
2. **Fix debug panel mobile dismiss** — Escape key + backdrop click
3. **Deduplicate tool.list calls** — single request per tab activation
