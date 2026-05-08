# Requirements: Cross-Platform Dioxus UI for Agent Interaction

## Background

The project currently has `vol-llm-tui`, a terminal UI built with ratatui for interacting with the ReAct coding agent. The user wants to migrate to a unified UI framework using Dioxus 0.6+ that compiles to both Web (WASM) and TUI targets, sharing the same component code. This enables a consistent experience across terminals and browsers, with a gradual migration path away from the existing ratatui-based TUI.

## Goals

1. **Unified component layer** — A single set of Dioxus components renders to both Web (WASM in browser) and TUI (terminal via crossterm/ratatui backend)
2. **Full feature parity** — All existing `vol-llm-tui` features are re-implemented: conversation view, workspace panel, tools panel, log viewer, session management, HITL approval, skills panel, status bar
3. **Dual communication modes** — The UI abstracts over local (in-process agent) and remote (HTTP/WS to agent service) communication through a single trait. TUI uses both LocalConnection and RemoteConnection; Web uses RemoteConnection only. The same Dioxus components consume a unified `UiEvent` type regardless of backend.
4. **Deployable remote agent service** — A standalone agent service that the Web WASM client connects to, built on `vol-llm-agent-channel` primitives
5. **Gradual migration** — The existing `vol-llm-tui` remains functional and can run in parallel. Migration is considered complete when the Dioxus TUI implements all features of the ratatui TUI and a user can replace `vol-llm-tui` with `vol-llm-ui` without losing functionality.

## Non-Goals

- We are NOT deleting `vol-llm-tui` — it stays as a fallback
- We are NOT building a server-side rendered (SSR) web app — Web target is WASM-only
- We are NOT changing agent logic, tool implementations, or the channel crate's core API
- We are NOT adding authentication to the remote agent service (handled later)

## Scope

### Included
- New crate `vol-llm-ui` with Dioxus 0.6+
- Shared Dioxus components: App shell, conversation view, workspace panel, tools panel, log viewer, session dialog, skills panel, status bar, input area, approval dialog
- Communication abstraction layer: `AgentConnection` trait with `LocalConnection` (in-process) and `RemoteConnection` (HTTP/WS) implementations
- TUI binary: supports local mode (default) and remote mode (`--remote <url>`)
- Web WASM binary: connects to remote agent service, configurable URL
- Remote agent service: deploys `vol-llm-agent-channel` with HTTP/WS endpoints, file access API for workspace and log browsing
- Auto-reconnect with exponential backoff for remote connections (max 5 retries, 1s-30s backoff)
- Runtime mode switching in TUI (switch between local and remote without restart, preserving current session)
- Workspace crate changes: add `vol-llm-ui` to workspace, add Dioxus dev-dependencies to channel examples if needed

### Excluded
- Server-side rendering (SSR) — Web is WASM client only
- Mobile native targets (iOS/Android)
- Desktop native targets (macOS/Windows/GTK) — these are future possibilities but not in scope
- Authentication/authorization for remote service
- Real-time collaboration (multiple users on same session)

## Constraints

- Dioxus 0.6+ latest version (uses rsx! macro, signals)
- Crate name: `vol-llm-ui`
- Must use existing `vol-llm-agent-channel` primitives for remote communication
- Must use existing `vol-llm-agent`, `vol-llm-agents`, `vol-session`, `vol-llm-observability`, `vol-llm-skill` for local mode
- WASM target must compile with `wasm32-unknown-unknown`
- Existing `vol-llm-tui` must continue to compile and run

## Success Criteria

| # | Criterion | Verification |
|---|-----------|-------------|
| 1 | `vol-llm-ui` compiles as native binary | `cargo build -p vol-llm-ui --bin vol-llm-ui` succeeds |
| 2 | `vol-llm-ui` compiles to WASM | `cargo build -p vol-llm-ui --target wasm32-unknown-unknown` succeeds |
| 3 | TUI local mode: send message, receive agent response | Start TUI, type "hello", agent replies within 30s |
| 4 | TUI remote mode: send message, receive agent response | Start TUI with `--remote <url>`, type message, response arrives via HTTP/WS |
| 5 | Web WASM loads and connects | Serve WASM files, open browser, UI renders within 5s, connects to remote service |
| 6 | All 7 panels functional in TUI local mode | Conversation, Workspace, Tools, Logs, Sessions, Skills, Status bar — each renders and interacts |
| 7 | All 7 panels functional in Web WASM mode | Same 7 panels render in browser and interact via remote service API |
| 8 | Communication abstraction enables component reuse | Swapping `LocalConnection` for `RemoteConnection` requires only config change; zero component code changes |
| 9 | Auto-reconnect on WS disconnect (≤5 retries, 1s-30s backoff) | Kill remote service, restart within 30s, connection restored automatically |
| 10 | Remote agent service is deployable | Runs as standalone binary serving HTTP/WS + file access endpoints |
| 11 | Existing `vol-llm-tui` still compiles | `cargo build -p vol-llm-tui` succeeds with no changes |
| 12 | TUI responsiveness comparable to existing ratatui TUI | Key press to render < 100ms, no dropped input |

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Local agent panics during run | Display error, restore input, allow retry |
| Remote WS drops during agent run | Auto-reconnect, restore session state, resume listening for events |
| Concurrent remote sessions | Each session isolated; server handles multiple agents independently |
| HITL approval timeout on remote | Agent waits for user response; UI shows pending state; configurable timeout |
| WASM file access | All file operations go through remote service HTTP API; no direct filesystem access |
| TUI mode switch at runtime | User can switch between local and remote without restart; state preserved for current session |
| Remote service unavailable at startup | Display connection error, show retry UI, auto-reconnect in background |
| Different event schemas between local and remote | Communication layer normalizes events to a single `UiEvent` type consumed by Dioxus components |

## Open Questions

1. **Dioxus TUI renderer availability (blocker for design phase).** Dioxus 0.6's terminal renderer (`dioxus-tui`) was experimental in 0.5 and may not be stable. **The design phase must prototype TUI rendering first** before committing to the shared-component architecture. If no viable TUI renderer exists, we fall back to: Dioxus for Web + ratatui for TUI, with a translation layer between Dioxus VNodes and ratatui widgets.

2. **Should the remote agent service support multiple concurrent agents per process, or one agent per process?** — The `vol-llm-agent-channel` examples already show both patterns (single_agent.rs, multi_agent.rs). We'll use the multi-agent pattern for the remote service to support multiple clients.

## Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                     vol-llm-ui                            │
│                                                           │
│  ┌────────────────────────────────────────────────────┐  │
│  │              Dioxus Components (shared)             │  │
│  │  AppShell, Conversation, Workspace, Tools, Logs,   │  │
│  │  Sessions, Skills, StatusBar, InputArea, Approval   │  │
│  └────────────────────────────────────────────────────┘  │
│                          │                                │
│  ┌────────────────────────────────────────────────────┐  │
│  │         Communication Abstraction Layer            │  │
│  │                                                    │  │
│  │  LocalConnection ────► ReActAgent (in-process)    │  │
│  │  RemoteConnection ──► HTTP/WS (agent service)     │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
         │                              │
    ┌────▼────┐                   ┌─────▼─────┐
    │ TUI bin │                   │ Web WASM  │
    │ native  │                   │ wasm32    │
    └─────────┘                   └───────────┘

┌──────────────────────────────────────────────────────────┐
│              Remote Agent Service (separate)              │
│                                                           │
│  vol-llm-agent-channel: HTTP/WS + AgentRouter            │
│  File access endpoints for workspace/log browsing        │
│  Multiple agents, multiple concurrent clients            │
└──────────────────────────────────────────────────────────┘
```
