---
name: vol-web-dev
description: Use when developing the React/Vite web frontend in the vol project — starting dev servers, adding Tailwind classes, debugging frontend-backend connection issues, or troubleshooting why styles don't appear
---

# Vol Web Development

## Overview

The web frontend is a React 18 + Vite + TypeScript app (`frontend/`) with shadcn/ui components and Jotai state management. It connects to a JSON-RPC agent backend over WebSocket. Two services run simultaneously: the Vite dev server (which includes Tailwind CSS via its Vite plugin) and the JSON-RPC backend.

## Architecture

```
Browser (port 5173)                  Backend (port 3001)
┌─────────────────────┐     WS      ┌──────────────────────┐
│ React SPA (Vite)    │◄───────────►│ JSON-RPC over WS     │
│ frontend/           │  ws://host  │ jsonrpc_agent_service│
│ JSON-RPC client     │  :3001      │ AgentServerCore      │
└─────────────────────┘             └──────────────────────┘
```

Vite proxies `/ws` to `ws://localhost:3001`, so the frontend connects to its own origin without cross-origin WebSocket issues.

## Project Structure

```
frontend/
├── src/
│   ├── main.tsx              # Entry point
│   ├── App.tsx               # Root component
│   ├── index.css             # Tailwind v4 + shadcn/ui dark theme
│   ├── components/
│   │   ├── dialogs/          # Overlay dialogs (ApprovalDialog, DebugPanel, ...)
│   │   ├── panels/           # Main panel views (ConversationView, NodesPanel, ...)
│   │   ├── inputs/           # Input components
│   │   ├── layout/           # Layout shell
│   │   ├── shared/           # Shared/reusable components
│   │   └── ui/               # shadcn/ui primitives (Button, Dialog, Select, ...)
│   ├── stores/               # Jotai atoms (connection.ts, conversation.ts, agents.ts, ...)
│   ├── hooks/                # Custom hooks (useAutoScroll, useThrottledValue)
│   ├── lib/                  # Utilities (jsonrpc-client.ts, ws-url.ts, protocol.ts, ...)
│   └── types/                # TypeScript type definitions
├── tests/                    # Test files
│   └── e2e/                  # Playwright e2e specs
├── vite.config.ts            # Vite config (plugins: react, tailwind; proxy /ws → :3001)
├── tsconfig.json             # TypeScript config
└── package.json              # Dependencies and scripts
```

## Startup

All commands run in separate terminals.

### Pre-flight: Check if Already Running

Before starting, check whether each service is already listening on its port:

```bash
# Check if Vite dev server is already running
lsof -i :5173 2>/dev/null && echo "Vite dev server already running" || echo "port 5173 free"

# Check if JSON-RPC backend is already running
lsof -i :3001 2>/dev/null && echo "backend already running" || echo "port 3001 free"
```

If a service is already running, don't start a duplicate. If a port is occupied by a stale process, kill it first: `kill $(lsof -ti :5173)`.

### Start Commands

```bash
# Terminal 1: Vite React dev server (includes Tailwind CSS via Vite plugin)
make web-dev

# Terminal 2: JSON-RPC backend
make web-backend
```

Tailwind CSS v4 is integrated via the `@tailwindcss/vite` plugin — **no separate CSS watch process is needed**. The Vite plugin handles CSS compilation and HMR automatically.

`make web-backend` uses `cargo watch`, which recompiles and restarts on any Rust source change in the workspace.

## Debugging

**Both services must be running to debug the full stack.** Missing either causes incomplete behavior:

| Service Down | Symptom |
|-------------|---------|
| `web-dev` | No frontend at all; browser can't load the page on port 5173 |
| `web-backend` | Agent panel shows "disconnected"; no agent interaction works |

**Debugging workflow:**

1. Run the pre-flight port checks above to confirm both are running
2. If a service is missing, start it in a new terminal
3. Check each terminal's output for errors — Vite prints TypeScript/import errors on change; `cargo watch` prints Rust compile errors
4. Open browser DevTools (F12):
   - **Console tab**: React errors, JSON-RPC client logs, connection status
   - **Network tab**: WebSocket connection status (`ws://host:5173/ws` proxied to `:3001`)
   - **React DevTools** (if installed): component tree and Jotai atom state
5. After fixing code, Vite HMR applies changes instantly for components/styles; the backend auto-reloads via `cargo watch`

## What Each Command Watches

| Command | Tool | Watches | Does NOT watch |
|---------|------|---------|----------------|
| `make web-dev` | `vite` | `frontend/src/**` (TSX, CSS, TS) | Backend |
| `make web-backend` | `cargo watch -x "run ..."` | All workspace crate sources | Nothing outside workspace |

Vite HMR handles CSS and component changes near-instantly. `make web-backend` auto-reloads on any Rust source change.

## Tailwind CSS (v4)

Tailwind CSS v4 is configured in `frontend/src/index.css` via `@import "tailwindcss"` and integrated through the `@tailwindcss/vite` Vite plugin. The plugin automatically scans all modules imported through Vite's module graph — **no manual `@source` directives needed**.

Custom theme tokens are defined with `@theme { ... }` in `index.css`. shadcn/ui CSS variables (HSL triples) are mapped via `@theme inline { ... }` for component compatibility.

## shadcn/ui Components

The project uses [shadcn/ui](https://ui.shadcn.com/docs/components) primitives built on Radix UI. Installed components live in `frontend/src/components/ui/`.

### Installed Primitives

| Component | Source | Usage |
|-----------|--------|-------|
| Button | `@/components/ui/button` | All buttons, with variants: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link`, `success` |
| Dialog | `@/components/ui/dialog` | All modal overlays |
| Tabs | `@/components/ui/tabs` | Tab navigation in main layout and sub-panels |
| Sheet | `@/components/ui/sheet` | Slide-in panels (CapabilityDrawer) |
| Switch | `@/components/ui/switch` | Toggle switches |
| Checkbox | `@/components/ui/checkbox` | Checkbox inputs |
| Input | `@/components/ui/input` | Text input fields |
| Select | `@/components/ui/select` | Dropdown selects |
| Label | `@/components/ui/label` | Form labels |
| Badge | `@/components/ui/badge` | Status/count indicators |
| ScrollArea | `@/components/ui/scroll-area` | Styled scrollable regions |
| Accordion | `@/components/ui/accordion` | Collapsible sections |
| Skeleton | `@/components/ui/skeleton` | Loading placeholders |
| Tooltip | `@/components/ui/tooltip` | Hover tooltips |

### Adding a New shadcn Component

```bash
cd frontend && npx shadcn@latest add <component-name>
```

Note: the CLI may write to a stray `frontend/@/` directory due to project-references tsconfig layout. Move files to `frontend/src/components/ui/` after install.

Available components: https://ui.shadcn.com/docs/components

### Conventions

- **Always use `<Button>`** — never raw `<button>` elements. Pick the closest variant. Add `cursor-pointer` in className (Tailwind v4 resets button cursor).
- **Always use `<ScrollArea>`** for scrollable panel containers — never raw `overflow-y-auto`.
- **Always use `<Badge>`** for status/count indicators — never raw styled `<span>` elements.
- **Loading states** — use `<Skeleton>` placeholders, not CSS spinner spans.
- **Dialogs** — use `<Dialog>` + `<DialogContent>`, not custom overlays.
- **Slide-in panels** — use `<Sheet>`, not custom fixed-position divs.

## Build and Check Commands

| Command | What it does |
|---------|-------------|
| `make web-check` | TypeScript type-check + Vite production build |
| `make web-build` | Production build (same as `make web-check`) |
| `make web-clippy` | TypeScript type-check only (`tsc -b --noEmit`) |

Use `make web-clippy` for fast iteration (type-check without building). Use `make web-check` or `make web-build` when you need the full production bundle in `frontend/dist/`.

For running tests:

```bash
npm --prefix frontend run test:run    # vitest unit tests (single run)
npm --prefix frontend run test        # vitest in watch mode
npx playwright test --config frontend/playwright.config.ts  # e2e tests
```

## Environment Variables

| Variable | Used by | Purpose |
|----------|---------|---------|
| `ANTHROPIC_AUTH_TOKEN` | `make web-backend` | API key for LLM provider (hardcoded to `sk` in Makefile) |
| `RUST_LOG` | `make web-backend` | Tracing filter level (defaults to `info`) |

## Common Mistakes

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| Page won't load on :5173 | Vite dev server not running | Start `make web-dev` |
| Agent panel shows "disconnected" | Backend not running on port 3001 | Start `make web-backend` |
| New Tailwind class has no effect | Vite HMR may have missed it; check for syntax errors in the class | Hard-refresh browser; verify class name is spelled correctly |
| TypeScript error on import | Path alias or missing type | `@/` alias maps to `frontend/src/` (configured in `vite.config.ts` and `tsconfig.json`) |
| Vite build fails but dev works | TypeScript strictness catches more in build mode | Run `make web-clippy` to see all type errors; fix before building |
| Backend change not reflected | `cargo watch` missed it or compile error | Check terminal output; restart `make web-backend` if needed |
| Port 5173/3001 already in use | Previous instance still running | Run pre-flight checks; kill stale process with `kill $(lsof -ti :5173)` |
| CSS variables not working | Tailwind v4 `@theme` config issue | Check `frontend/src/index.css` for correct `@theme` / `@theme inline` blocks |
| shadcn/ui component broken | Missing CSS variable or Radix dependency | Verify all `@radix-ui/*` deps are installed and CSS variables are defined in `index.css` |

## Adding New Dependencies

When adding a new npm dependency:

```bash
npm --prefix frontend install <package>
```

- Ensure the package is compatible with the React 18 / Vite 6 / TypeScript 5.6 toolchain
- Run `make web-check` to verify the build still passes
- After installing, `make web-dev` picks up new deps automatically (Vite handles dependency changes)
- `make web-backend` is unaffected by frontend dependency changes

When adding a new Rust crate dependency (for backend changes):
- Add to the relevant `Cargo.toml` in the workspace
- Restart `make web-backend` manually — `cargo watch` does NOT pick up new Cargo.toml dependencies automatically
