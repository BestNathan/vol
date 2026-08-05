# Dialog Animation & UX Unification

**Date**: 2026-08-05
**Status**: design-approved
**Scope**: `crates/vol-llm-ui`

## Problem

All 15 overlay components (12 centered modals, 2 side drawers, 1 dropdown) appear and disappear
instantly with no transition. They use Dioxus conditional rendering (`if !open { return rsx! {} }`),
which removes elements from DOM immediately — CSS transitions are impossible on removed elements.

User feedback: dialogs feel like they "pop from top-left abruptly."

Secondary issues:
- **No shared base component** — each dialog duplicates overlay backdrop + positioning + close logic
- **Inconsistent styling** — 4 different z-indexes, 3 different backdrop opacities, 4 different close button characters, 4 different border colors

## Design

### Animation style

A) **Scale + fade** (macOS/iOS style): `scale(0.95) → scale(1)` + `opacity(0 → 1)` with slight upward drift (`translateY(-8px)`).

Exit: reverse — `scale(1) → scale(0.95)` + `opacity(1 → 0)`.

### Timing

| Phase | Duration | Easing | 
|-------|----------|--------|
| Backdrop enter | 150ms | ease-out |
| Dialog enter | 200ms | ease-out |
| Backdrop exit | 150ms | ease-in |
| Dialog exit | 150ms | ease-in |

### Architecture

New shared component `AnimatedOverlay`:

```
crates/vol-llm-ui/src/web/components/animated_overlay.rs  ← new
crates/vol-llm-ui/assets/input.css                         ← new @keyframes
```

**Component API:**
```rust
#[component]
pub fn AnimatedOverlay(
    open: bool,                 // external open/close signal
    on_close: EventHandler<()>, // close callback (backdrop click)
    children: Element,          // dialog content
) -> Element
```

**State machine:**
```
Hidden ──open=true──▶ Entering ──200ms timeout──▶ Visible
                                                     │
                                          open=false │ or backdrop click
                                                     ▼
Hidden ◀──150ms timeout── Exiting ◀─────────────────┘
```

Behaviors:
- Backdrop click → triggers `on_close`
- Escape key → triggers `on_close` (added to all overlays)
- During Entering/Exiting phases, backdrop clicks are ignored (prevents double-close race)
- Default: backdrop click closes. For dialogs that shouldn't close on backdrop (ToolCallDialog, DebugPanel), we add a `close_on_backdrop: bool` prop

### CSS animations

Added to `assets/input.css`:

```css
@keyframes overlay-fade-in {
  from { opacity: 0; }
  to   { opacity: 1; }
}
@keyframes overlay-fade-out {
  from { opacity: 1; }
  to   { opacity: 0; }
}
@keyframes dialog-enter {
  from { opacity: 0; transform: scale(0.95) translateY(-8px); }
  to   { opacity: 1; transform: scale(1) translateY(0); }
}
@keyframes dialog-exit {
  from { opacity: 1; transform: scale(1) translateY(0); }
  to   { opacity: 0; transform: scale(0.95) translateY(-8px); }
}
```

Tailwind custom utilities (in `@theme` block):
```css
--animate-overlay-in:  overlay-fade-in  150ms ease-out both;
--animate-overlay-out: overlay-fade-out 150ms ease-in  both;
--animate-dialog-in:   dialog-enter     200ms ease-out both;
--animate-dialog-out:  dialog-exit      150ms ease-in  both;
```

### z-index normalization

| Layer | z-index | Usage |
|-------|---------|-------|
| Backdrop | `z-40` | Shared overlay backdrop |
| Modal content | `z-50` | Centered dialogs |
| High-priority | `z-[100]` | Approval dialog, session detail viewer |

This eliminates the current chaos: `z-40`, `z-50`, `z-[100]`, `z-[200]`.

### Component tree after migration

In `app.rs`, all app-level dialogs rendered as siblings outside layout containers:
```
div.main-layout
  ...panels...
AnimatedOverlay { ApprovalDialog }
AnimatedOverlay { ToolCallDialog }
AnimatedOverlay { ResourceViewer }
AnimatedOverlay { PromptViewer }
AnimatedOverlay { SkillDetailDialog }
AnimatedOverlay { DebugPanel }
CapabilityDrawer  (separate component, slide animation)
```

Panel-local dialogs (SystemToolDialog, ContextDialog, ToolDetailModal, SessionDetailOverlay, TaskDepGraph) each wrap their content in `AnimatedOverlay` within their parent component.

## Implementation plan

### Step 1: Create `AnimatedOverlay` + CSS animations (new files only, zero impact on existing code)

File: `crates/vol-llm-ui/src/web/components/animated_overlay.rs`
- Implement `OverlayPhase` enum and state machine
- Use `gloo_timers::future::TimeoutFuture` for phase transitions
- Render backdrop div + content wrapper div with conditional `animate-*` classes
- Handle `open` prop changes to trigger enter/exit

File: `crates/vol-llm-ui/assets/input.css`
- Add 4 `@keyframes` definitions
- Add 4 `--animate-*` custom utilities in `@theme` block

### Step 2: Migrate centered modals (one component at a time)

Each migration replaces the outer overlay div pattern:
```diff
- div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
-     onclick: move |_| { signal.write_unchecked().open = false; },
-     div { class: "bg-[#1a1a2e] border ... rounded-lg",
-         onclick: move |evt| evt.stop_propagation(),
+ AnimatedOverlay {
+     open: s.open,
+     on_close: move |_| signal.write_unchecked().open = false,
      div { class: "bg-[#1a1a2e] border ... rounded-lg",
          ...content unchanged...
      }
-     }
  }
+ }
```

Migration order (simplest to most complex):
1. `mcp_resource_viewer.rs` — simplest, few props
2. `mcp_prompt_viewer.rs` — same pattern
3. `mcp_tool_dialog.rs` — similar
4. `tool_dialog.rs` (SystemToolDialog) — panel-local
5. `context_panel.rs` (ContextDialog) — panel-local
6. `skill_detail_dialog.rs`
7. `conversation.rs` (ToolDetailModal) — panel-local
8. `approval_dialog.rs`
9. `debug_panel.rs`
10. `task_dep_graph.rs`
11. `sessions_panel.rs` (SessionDetailOverlay)

### Step 3: Side drawer animation

`capability_drawer.rs` — use the same state machine but with `translateX` slide animation instead of scale:
```css
@keyframes drawer-enter {
  from { transform: translateX(100%); }
  to   { transform: translateX(0); }
}
@keyframes drawer-exit {
  from { transform: translateX(0); }
  to   { transform: translateX(100%); }
}
```

File tree mobile drawer — same `translateX` pattern for left-side drawer.

### Step 4: Remove dead code

- `session_dialog.rs` — `SessionDialog` component is never rendered anywhere, remove it
- Clean up unused `SessionDialogState` in `state/mod.rs`

## Testing

- Visual verification: use `make web-dev` to serve the frontend, open each dialog type
- Verify enter animation plays (200ms scale+fade)
- Verify exit animation plays (150ms scale+fade reverse)
- Verify backdrop click dismisses
- Verify Escape key dismisses
- Verify rapid open/close doesn't leave stale state
- Verify no z-index stacking issues with multiple dialogs

## Out of scope

- `NodesDropdown` — this is a menu/popover, not a modal. Its animation pattern (hover/focus-triggered) is different. Handle separately later.
- Mobile-specific behavior — current breakpoints already handle mobile sizing; animation remains the same
