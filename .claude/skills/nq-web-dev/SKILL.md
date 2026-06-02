---
name: nq-web-dev
description: Use when developing the vol-llm-ui Dioxus/WASM web frontend in the nq-deribit project — starting dev servers, adding Tailwind classes, debugging frontend-backend connection issues, or troubleshooting why styles don't appear
---

# NQ Web Development

## Overview

The web frontend is a Dioxus 0.6 WASM app (`crates/vol-llm-ui`) that connects to a JSON-RPC agent backend over WebSocket. Three watch-mode services must run simultaneously: Tailwind CSS, Dioxus dev server, and the JSON-RPC backend.

## Architecture

```
Browser (port 8080)                  Backend (port 3001)
┌─────────────────────┐     WS      ┌──────────────────────┐
│ Dioxus WASM (web)   │◄───────────►│ JSON-RPC over WS     │
│ vol-llm-ui          │  ws://host  │ jsonrpc_agent_service│
│ AgentConnection     │  :3001      │ AgentServerCore      │
└─────────────────────┘             └──────────────────────┘
```

The frontend uses `AgentConnection` trait with two implementations:
- `RemoteConnection` — WebSocket JSON-RPC client (production/dev)
- `LocalConnection` — in-process agent (alternative)

In dev mode, the frontend connects via `RemoteConnection` to `ws://<host>:3001`.

## Startup

All three commands run in separate terminals. **Order matters.**

### Pre-flight: Check if Already Running

Before starting, check whether each service is already listening on its port:

```bash
# Check if Dioxus dev server is already running
lsof -i :8080 2>/dev/null && echo "dev server already running" || echo "port 8080 free"

# Check if JSON-RPC backend is already running
lsof -i :3001 2>/dev/null && echo "backend already running" || echo "port 3001 free"
```

`make web-css` has no fixed port — check with `pgrep -f tailwindcss`.

If a service is already running, don't start a duplicate. If a port is occupied by a stale process, kill it first: `kill $(lsof -ti :8080)`.

### Start Commands

```bash
# Terminal 1: Tailwind CSS watch (MUST start first)
make web-css

# Terminal 2: Dioxus dev server (MUST start after web-css is running)
make web-dev

# Terminal 3: JSON-RPC backend (can start anytime)
make web-backend
```

`make web-css` compiles `assets/input.css` → `assets/tailwind.css` in watch mode. If it's not running when `make web-dev` starts, new Tailwind classes (especially arbitrary values like `w-[600px]`) won't be in the compiled CSS and won't take effect.

## Debugging

**All three services must be running to debug the full stack.** Missing any one causes incomplete behavior:

| Service Down | Symptom |
|-------------|---------|
| `web-css` | New Tailwind classes don't take effect; arbitrary values like `w-[600px]` are ignored |
| `web-dev` | No frontend at all; browser can't load the page on port 8080 |
| `web-backend` | Agent panel shows "disconnected"; no agent interaction works |

**Debugging workflow:**

1. Run the pre-flight port checks above to confirm all three are running
2. If any service is missing, start it in a new terminal
3. Check each terminal's output for compile errors — `cargo watch` and `dx serve` both print errors on change
4. Open browser DevTools (F12): check the Console tab for WASM panics and the Network tab for WebSocket connection status to `ws://<host>:3001`
5. After fixing code, verify the relevant watch process picked up the change (look for recompilation output in its terminal)

## What Each Command Watches

| Command | Tool | Watches | Does NOT watch |
|---------|------|---------|----------------|
| `make web-css` | `@tailwindcss/cli --watch` | `assets/input.css`, files matched by `@source` globs | Nothing else |
| `make web-dev` | `dx serve` | `crates/vol-llm-ui/src/**` (Rust source) | CSS output, backend |
| `make web-backend` | `cargo watch -x "run ..."` | All workspace crate sources | Nothing outside workspace |

`make web-backend` **does** auto-reload — it uses `cargo watch`, which recompiles and restarts on any source change in the workspace dependency tree.

## Tailwind Scanning

The Tailwind v4 setup scans Rust component files for class names. In `assets/input.css`:

```css
@import "tailwindcss";
@source "../src/web/components/*.rs";
```

Only files matching `crates/vol-llm-ui/src/web/components/*.rs` are scanned. Classes used in other directories (e.g., `src/web/bin/`, `src/tui/`, `src/connection/`) will NOT be picked up unless a `@source` directive covers them.

**Adding a new component directory?** Add a corresponding `@source` line to `input.css`.

## Build and Check Commands

| Command | What it does | WASM target? |
|---------|-------------|--------------|
| `make web-check` | `cargo check` with `--features web` | No — checks native compilation only |
| `make web-build` | Full WASM build | Yes — `--target wasm32-unknown-unknown` |
| `make web-clippy` | Clippy with `--features web`, `-D warnings` | No |

Use `make web-check` for fast iteration (compile-check without WASM target overhead). Use `make web-build` when you need the actual WASM binary. Use `make web-clippy` before landing changes.

## Environment Variables

| Variable | Used by | Purpose |
|----------|---------|---------|
| `ANTHROPIC_AUTH_TOKEN` | `make web-backend` | API key for LLM provider (hardcoded to `sk` in Makefile) |
| `RUST_LOG` | `make web-backend` | Tracing filter level (defaults to `info`) |

## Common Mistakes

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| New Tailwind class has no effect | `make web-css` not running before `make web-dev` | Restart both in correct order |
| Class used outside `src/web/components/` | Not in `@source` glob | Add `@source` directive to `input.css` |
| Agent panel shows "disconnected" | Backend not running on port 3001 | Start `make web-backend` |
| Backend change not reflected | `cargo watch` missed it or compile error | Check terminal output; `cargo watch` auto-reloads on any workspace source change |
| WASM build errors but `web-check` passes | `web-check` doesn't target WASM | Use `make web-build` for WASM-specific checks |
| `dx serve` not found | Dioxus CLI not installed | `cargo install dioxus-cli` |
| Port 8080/3001 already in use | Previous instance still running | Run pre-flight checks; kill stale process with `kill $(lsof -ti :8080)` |
| Debugging but something not working | Not all three services running | Run pre-flight checks to verify all three are up |

## Adding New Dependencies

When adding a new crate dependency to `crates/vol-llm-ui/Cargo.toml`:
- Ensure it compiles to `wasm32-unknown-unknown` (no native-only deps)
- `cargo watch` on the backend does NOT pick up new Cargo.toml changes automatically — restart `make web-backend` manually after `cargo update`
- Run `make web-check` first, then `make web-build` to verify WASM compatibility
