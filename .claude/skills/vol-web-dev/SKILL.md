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

The project uses [shadcn/ui](https://ui.shadcn.com/docs/components) primitives built on **Radix UI** (`base: radix`). Installed components live in `frontend/src/components/ui/`. Config is in `frontend/components.json`.

### Project Context

Run `cd frontend && npx shadcn@latest info` to refresh. Key fields:
- `base: radix` — use `asChild` (not `render`) for custom triggers
- `iconLibrary: lucide` — import from `lucide-react`
- `tailwindVersion: v4` — `@theme` blocks, no `tailwind.config.js`
- `aliases.ui: @/components/ui` — import path for all UI primitives

### Installed Primitives

| Component | Source | Usage |
|-----------|--------|-------|
| Button | `@/components/ui/button` | All buttons, variants: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link`, `success` |
| Dialog | `@/components/ui/dialog` | All modal overlays (custom: `hideCloseButton`, `overlayClassName` props) |
| Tabs | `@/components/ui/tabs` | Tab navigation in main layout and sub-panels |
| Sheet | `@/components/ui/sheet` | Slide-in panels (CapabilityDrawer) |
| Switch | `@/components/ui/switch` | Toggle switches |
| Checkbox | `@/components/ui/checkbox` | Checkbox inputs |
| Input | `@/components/ui/input` | Text input fields |
| Select | `@/components/ui/select` | Dropdown selects (use `SelectGroup` + `SelectItem`) |
| Label | `@/components/ui/label` | Form labels |
| Badge | `@/components/ui/badge` | Status/count indicators |
| ScrollArea | `@/components/ui/scroll-area` | Styled scrollable regions |
| Accordion | `@/components/ui/accordion` | Collapsible sections |
| Skeleton | `@/components/ui/skeleton` | Loading placeholders |
| Tooltip | `@/components/ui/tooltip` | Hover tooltips |
| Empty | `@/components/ui/empty` | Empty states (`EmptyHeader`, `EmptyTitle`, `EmptyDescription`, `EmptyMedia`, `EmptyContent`) |
| Separator | `@/components/ui/separator` | Visual dividers (horizontal/vertical) |

### Adding a New shadcn Component

```bash
cd frontend && npx shadcn@latest add <component-name>
```

**CRITICAL:** The CLI may write files to `frontend/@/components/ui/` instead of `frontend/src/components/ui/` due to the `@` import alias. After install, check `ls frontend/@/components/ui/` — if files exist there, move them:
```bash
mv frontend/@/components/ui/<file>.tsx frontend/src/components/ui/
rm -rf frontend/@/
```

Search for components: `npx shadcn@latest search -q "<query>"`
Get docs: `npx shadcn@latest docs <component>`
Preview before overwriting: `npx shadcn@latest add <component> --dry-run --diff`

### Coding Rules (enforced in review)

These rules mirror the shadcn skill's critical rules. Violations block PRs.

#### Spacing
- **Use `flex` with `gap-*`** for all spacing. Never `space-y-*` or `space-x-*`.
  ```tsx
  // ✅ correct
  <div className="flex flex-col gap-4">
  // ❌ wrong
  <div className="space-y-4">
  ```
- Exception: `space-y-0` is acceptable to override inherited spacing on a specific child.

#### Icons (lucide-react)
- **Icons in `Button`: use `data-icon` attribute.** No sizing classes on the icon.
  ```tsx
  // ✅ correct
  <Button>
    <SearchIcon data-icon="inline-start" />
    Search
  </Button>
  // ❌ wrong
  <Button>
    <SearchIcon className="h-4 w-4" />
    Search
  </Button>
  ```
- Standalone icons (not in buttons) can use `className="h-4 w-4"`.
- Use `size-*` when width and height are equal: `size-10` not `w-10 h-10`.

#### Text Truncation
- **Use `truncate` shorthand.** Never manual `overflow-hidden text-ellipsis whitespace-nowrap`.
  ```tsx
  // ✅ correct
  <span className="truncate">
  // ❌ wrong
  <span className="overflow-hidden text-ellipsis whitespace-nowrap">
  ```

#### Conditional Classes
- **Use `cn()` from `@/lib/utils`** for all conditional class merging. Never template literals.
  ```tsx
  // ✅ correct
  className={cn("base-class", isActive && "active-class")}
  // ❌ wrong
  className={`base-class ${isActive && "active-class"}`}
  ```

#### Component Composition
- **Items always inside their Group.** `SelectItem` → `SelectGroup`. `DropdownMenuItem` → `DropdownMenuGroup`.
  ```tsx
  // ✅ correct
  <SelectContent>
    <SelectGroup>
      <SelectItem value="a">A</SelectItem>
    </SelectGroup>
  </SelectContent>
  ```
- **Dialog, Sheet, Drawer always need a Title.** `DialogTitle`, `SheetTitle` required for accessibility. Use `className="sr-only"` if visually hidden.
- **Button has no `isPending`/`isLoading`.** Compose with `Spinner` + `data-icon` + `disabled`.
- **`TabsTrigger` must be inside `TabsList`.** Never render triggers directly in `Tabs`.
- **`Avatar` always needs `AvatarFallback`.**

#### Use Components, Not Raw Markup
- **Empty states** → `<Empty>` + `<EmptyHeader>` + `<EmptyTitle>`, not custom centered divs.
- **Dividers** → `<Separator />`, not `<hr>` or `<div className="border-t">`.
- **Status badges** → `<Badge variant="secondary">`, not raw `<span>`.
- **Loading** → `<Skeleton>`, not custom `animate-pulse` divs.
- **Callouts** → `<Alert>`, not custom styled divs.
- **Toast** → `toast()` from `sonner` (Radix projects).

#### Colors
- **Use semantic tokens** (`bg-primary`, `text-muted-foreground`, `bg-destructive`). Never raw Tailwind colors (`bg-blue-500`, `text-emerald-400`).
- **Exception:** Status result boxes (success/error) may use `bg-emerald-950/30 border-emerald-500/50` and `bg-red-950/30 border-destructive/50` as these are approved patterns for result display.
- **Button variants** use semantic tokens: the `success` variant uses `bg-success text-success-foreground` (defined in `index.css`), not `bg-emerald-600`.

#### Overlays
- **No manual `z-index`** on Dialog, Sheet, Popover — they handle their own stacking via `z-50`.
- **No `dark:` color overrides** — use semantic tokens that work in both themes.

#### Forms
- The project uses a custom `SchemaForm` (JSON Schema → form) with `Input`, `Select`, `Checkbox`, `Label`. When adding net-new forms, follow the existing SchemaForm patterns.
- If adding static forms, use `FieldGroup` + `Field` pattern (requires installing the `field` component).

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
| shadcn CLI writes to wrong path | `@` alias resolves to `frontend/@/` in CLI context | After `npx shadcn@latest add`, check `frontend/@/` and move files to `frontend/src/components/ui/` |
| New shadcn component not found | Import path wrong or component not installed | Use CLI to add: `cd frontend && npx shadcn@latest add <name>`; never create UI primitives manually |
| `space-y-*` used in new code | Violates shadcn spacing rule | Use `flex flex-col gap-*` instead; run `grep -rn 'space-y-\|space-x-' src/` to check |
| Icons in Button missing `data-icon` | Violates shadcn icon rule | Add `data-icon="inline-start"` or `data-icon="inline-end"` on the icon element |

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
