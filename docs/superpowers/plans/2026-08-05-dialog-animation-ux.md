# Dialog Animation & UX Unification — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 15 instant-appearing overlay components with animated, consistent UX using a shared `AnimatedOverlay` component.

**Architecture:** New `AnimatedOverlay` component wraps dialog content with a state-machine-driven overlay (backdrop + centered content wrapper). CSS `@keyframes` handle scale+fade animations. Each existing dialog drops its outer backdrop div and uses `AnimatedOverlay` instead. The `CapabilityDrawer` gets its own slide animation using the same state-machine pattern.

**Tech Stack:** Dioxus 0.6 signals, `gloo_timers::future::TimeoutFuture`, Tailwind v4 CSS `@theme` custom animations, `wasm_bindgen_futures::spawn_local`

## Global Constraints

- No doc tests — write `#[cfg(test)]` unit tests or `tests/` integration tests
- Every new `pub fn` → at least one test
- `main.rs`, `app.rs`, `health.rs` exempt from coverage
- Docker builds use `rsproxy.cn` mirror — copy `.cargo/config.toml` into builder stage
- Web frontend: use `make web-*` commands; never `cargo build/run` directly for vol-llm-ui

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-ui/assets/input.css` | Modify | Add 4 `@keyframes` + 4 `--animate-*` custom utilities |
| `crates/vol-llm-ui/src/web/components/animated_overlay.rs` | **Create** | Shared `AnimatedOverlay` component with `OverlayPhase` state machine |
| `crates/vol-llm-ui/src/web/components/mod.rs` | Modify | Add `pub mod animated_overlay` + re-export |
| `crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs` | Modify | Migrate to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs` | Modify | Migrate to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs` | Modify | Migrate to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/tool_dialog.rs` | Modify | Migrate SystemToolDialog to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/context_panel.rs` | Modify | Migrate ContextDialog to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs` | Modify | Migrate to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/conversation.rs` | Modify | Migrate ToolDetailModal to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/approval_dialog.rs` | Modify | Migrate to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/debug_panel.rs` | Modify | Migrate to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/task_dep_graph.rs` | Modify | Migrate to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/sessions_panel.rs` | Modify | Migrate SessionDetailOverlay to AnimatedOverlay |
| `crates/vol-llm-ui/src/web/components/capability_drawer.rs` | Modify | Add slide-in/out animation (own state machine) |
| `crates/vol-llm-ui/src/web/components/session_dialog.rs` | **Delete** | Dead code — never rendered |
| `crates/vol-llm-ui/src/web/components/mod.rs` | Modify | Remove `pub mod session_dialog` + its re-export |
| `crates/vol-llm-ui/src/state/mod.rs` | Modify | Remove unused `SessionDialogState` (if unreferenced) |

---

### Task 1: CSS animations in input.css

**Files:**
- Modify: `crates/vol-llm-ui/assets/input.css`

**Interfaces:**
- Produces: CSS classes `animate-overlay-in`, `animate-overlay-out`, `animate-dialog-in`, `animate-dialog-out` — consumed by `AnimatedOverlay` component (Task 2)

- [ ] **Step 1: Add @keyframes and @theme animate utilities**

In `crates/vol-llm-ui/assets/input.css`, add after the existing `conn-blink` keyframe block:

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

Then add to the existing `@theme` block:

```css
--animate-overlay-in:  overlay-fade-in  150ms ease-out both;
--animate-overlay-out: overlay-fade-out 150ms ease-in  both;
--animate-dialog-in:   dialog-enter     200ms ease-out both;
--animate-dialog-out:  dialog-exit      150ms ease-in  both;
```

- [ ] **Step 2: Rebuild CSS to verify Tailwind compiles the new utilities**

Run: `make web-css`
Expected: `tailwind.css` regenerated with no errors. Verify the new `@keyframes` and `.animate-*` classes appear in the output.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/assets/input.css crates/vol-llm-ui/assets/tailwind.css
git commit -m "feat(ui): add dialog scale+fade CSS animations"
```

---

### Task 2: AnimatedOverlay component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/animated_overlay.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

**Interfaces:**
- Produces: `AnimatedOverlay` component — consumed by Tasks 3–13
- Produces: `OverlayPhase` enum (pub(crate))
- Produces: `fn next_phase(current: OverlayPhase, open: bool) -> OverlayPhase` (testable pure function)

- [ ] **Step 1: Write unit tests for the phase transition logic**

In `animated_overlay.rs`, write the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_to_entering_on_open() {
        assert_eq!(
            next_phase(OverlayPhase::Hidden, true),
            OverlayPhase::Entering
        );
    }

    #[test]
    fn hidden_stays_hidden_when_closed() {
        assert_eq!(
            next_phase(OverlayPhase::Hidden, false),
            OverlayPhase::Hidden
        );
    }

    #[test]
    fn entering_to_exiting_when_closed_during_enter() {
        assert_eq!(
            next_phase(OverlayPhase::Entering, false),
            OverlayPhase::Exiting
        );
    }

    #[test]
    fn entering_stays_entering_while_open() {
        assert_eq!(
            next_phase(OverlayPhase::Entering, true),
            OverlayPhase::Entering
        );
    }

    #[test]
    fn visible_to_exiting_on_close() {
        assert_eq!(
            next_phase(OverlayPhase::Visible, false),
            OverlayPhase::Exiting
        );
    }

    #[test]
    fn visible_stays_visible_while_open() {
        assert_eq!(
            next_phase(OverlayPhase::Visible, true),
            OverlayPhase::Visible
        );
    }

    #[test]
    fn exiting_to_entering_when_reopened() {
        assert_eq!(
            next_phase(OverlayPhase::Exiting, true),
            OverlayPhase::Entering
        );
    }

    #[test]
    fn exiting_to_hidden_when_closed() {
        // After timeout fires, caller sets Hidden. The pure fn just
        // stays in Exiting if open remains false.
        assert_eq!(
            next_phase(OverlayPhase::Exiting, false),
            OverlayPhase::Exiting
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-ui -- animated_overlay`
Expected: FAIL — module/type/enum not defined yet

- [ ] **Step 3: Implement OverlayPhase enum, next_phase, and AnimatedOverlay component**

Write the full file `crates/vol-llm-ui/src/web/components/animated_overlay.rs`:

```rust
//! Shared animated overlay for dialogs, modals, and popups.
//!
//! Wraps dialog content with a backdrop + centered container that
//! animates in/out with scale+fade transitions. Handles backdrop-click
//! dismissal and prevents double-close races via a state machine.

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

/// Lifecycle phase of the overlay.
#[derive(PartialEq, Clone, Copy, Debug)]
pub(crate) enum OverlayPhase {
    Hidden,
    Entering,
    Visible,
    Exiting,
}

/// Pure state transition: given current phase and desired `open` state,
/// return the next phase. Timeout side effects are handled by the component.
pub(crate) fn next_phase(current: OverlayPhase, open: bool) -> OverlayPhase {
    use OverlayPhase::*;
    match (current, open) {
        // Open requested — start entering
        (Hidden, true) => Entering,
        // Re-open during exit — restart enter animation
        (Exiting, true) => Entering,
        // Close requested during enter or visible — start exiting
        (Entering, false) => Exiting,
        (Visible, false) => Exiting,
        // No change
        _ => current,
    }
}

/// Animated overlay for centered modal dialogs.
///
/// ```text
/// State machine:
///   Hidden ──open=true──▶ Entering ──200ms timeout──▶ Visible
///                                                         │
///                                              open=false │ or backdrop click
///                                                         ▼
///   Hidden ◀──150ms timeout── Exiting ◀──────────────────┘
/// ```
#[component]
pub fn AnimatedOverlay(
    /// Whether the overlay should be open. External control.
    open: bool,
    /// Called when the user clicks the backdrop (not when `open` becomes false externally).
    on_close: EventHandler<()>,
    /// Dialog content (card, panel, etc.).
    children: Element,
) -> Element {
    let mut phase: Signal<OverlayPhase> = use_signal(|| OverlayPhase::Hidden);

    // React to `open` prop changes: kick off enter or exit animation.
    use_effect(move || {
        let next = next_phase(*phase.read(), open);
        if next == *phase.read() {
            return;
        }
        phase.set(next);

        match next {
            OverlayPhase::Entering => {
                let mut ph = phase;
                wasm_bindgen_futures::spawn_local(async move {
                    TimeoutFuture::new(200).await;
                    ph.with_mut(|p| {
                        if *p == OverlayPhase::Entering {
                            *p = OverlayPhase::Visible;
                        }
                    });
                });
            }
            OverlayPhase::Exiting => {
                let mut ph = phase;
                wasm_bindgen_futures::spawn_local(async move {
                    TimeoutFuture::new(150).await;
                    ph.set(OverlayPhase::Hidden);
                });
            }
            _ => {}
        }
    });

    let current = *phase.read();
    if current == OverlayPhase::Hidden {
        return rsx! {};
    }

    let backdrop_class = match current {
        OverlayPhase::Entering => {
            "fixed inset-0 bg-black/50 z-40 flex items-center justify-center p-4 animate-overlay-in"
        }
        OverlayPhase::Visible => {
            "fixed inset-0 bg-black/50 z-40 flex items-center justify-center p-4"
        }
        OverlayPhase::Exiting => {
            "fixed inset-0 bg-black/50 z-40 flex items-center justify-center p-4 animate-overlay-out"
        }
        _ => "",
    };

    let wrapper_class = match current {
        OverlayPhase::Entering => "animate-dialog-in",
        OverlayPhase::Exiting => "animate-dialog-out",
        _ => "",
    };

    rsx! {
        div {
            class: "{backdrop_class}",
            onclick: move |_| {
                if *phase.read() == OverlayPhase::Visible {
                    phase.set(OverlayPhase::Exiting);
                    let mut ph = phase;
                    let cb = on_close.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        TimeoutFuture::new(150).await;
                        ph.set(OverlayPhase::Hidden);
                        cb.call(());
                    });
                }
            },
            // Wrapper: stops click propagation so clicking the dialog
            // content does not dismiss. Also carries the enter/exit animation.
            div {
                class: "{wrapper_class}",
                onclick: move |evt: Event<MouseData>| evt.stop_propagation(),
                {children}
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-ui -- animated_overlay`
Expected: all 8 tests PASS

- [ ] **Step 5: Register the module**

In `crates/vol-llm-ui/src/web/components/mod.rs`:

Add after the `pub mod agents_panel;` line:
```rust
pub mod animated_overlay;
```

Add after the `pub use agents_panel::AgentsPanel;` line:
```rust
pub use animated_overlay::AnimatedOverlay;
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/animated_overlay.rs \
        crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat(ui): add AnimatedOverlay shared component with scale+fade animation"
```

---

### Task 3: Migrate MCP dialogs (resource_viewer, prompt_viewer, tool_dialog)

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` component from Task 2

All three use the same pattern: `McpDialogState` signal, app-level mount, no backdrop-close. Each has a field (`resource_viewer`, `prompt_viewer`, `tool_call_dialog`) that is `Option<T>`. When `None`, the dialog is hidden.

- [ ] **Step 1: Migrate mcp_resource_viewer.rs**

Change the outer `div` (lines 23-24) to wrap content in `AnimatedOverlay`:

```rust
use super::animated_overlay::AnimatedOverlay;

// In the component body, replace lines 23-69 with:
let open = signal.read().resource_viewer.is_some();

rsx! {
    AnimatedOverlay {
        open,
        on_close: move |_| { signal.write_unchecked().resource_viewer = None; },
        div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[500px] max-w-[90vw] max-h-[80vh] flex flex-col",
            // ... header, content unchanged ...
        }
    }
}
```

Remove the `onclick: stop_propagation()` from the inner card div (it's now handled by AnimatedOverlay's wrapper).

Remove the `onclick` on the outer backdrop div (now handled by AnimatedOverlay).

- [ ] **Step 2: Migrate mcp_prompt_viewer.rs**

Same pattern — replace the outer `fixed inset-0` backdrop div with `AnimatedOverlay`:

```rust
use super::animated_overlay::AnimatedOverlay;

// open = signal.read().prompt_viewer.is_some()
// on_close sets signal.write_unchecked().prompt_viewer = None
```

- [ ] **Step 3: Migrate mcp_tool_dialog.rs**

Same pattern — replace the outer `fixed inset-0` backdrop div with `AnimatedOverlay`:

```rust
use super::animated_overlay::AnimatedOverlay;

// open = signal.read().tool_call_dialog.is_some()
// on_close sets signal.write_unchecked().tool_call_dialog = None
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs \
        crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs \
        crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs
git commit -m "refactor(ui): migrate MCP dialogs to AnimatedOverlay"
```

---

### Task 4: Migrate SystemToolDialog

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tool_dialog.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

The `SystemToolDialog` has its own state struct `SystemToolDialogState` with an `open: bool` field. Signal is created locally in `tools_tab.rs` and passed as prop.

- [ ] **Step 1: Wrap content in AnimatedOverlay**

```rust
use super::animated_overlay::AnimatedOverlay;

// Replace the outer backdrop div (lines 63-66) with:
rsx! {
    AnimatedOverlay {
        open: s.open,
        on_close: move |_| { signal.write_unchecked().open = false; },
        div {
            class: "w-[95vw] sm:w-[600px] max-h-[85vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
            // header, content unchanged
        }
    }
}
```

Remove the `onclick: stop_propagation()` from the inner card (line 69).

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tool_dialog.rs
git commit -m "refactor(ui): migrate SystemToolDialog to AnimatedOverlay"
```

---

### Task 5: Migrate ContextDialog

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/context_panel.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

`ContextDialog` is a private component inside `context_panel.rs`. It has `on_close: EventHandler<()>` already.

- [ ] **Step 1: Wrap in AnimatedOverlay**

Replace lines 37-79 (the outer backdrop div and card wrapper):

```rust
use super::animated_overlay::AnimatedOverlay;

rsx! {
    AnimatedOverlay {
        open: true,   // This component only renders when dialog_open is true
        on_close: move |_| on_close.call(()),
        div {
            class: "w-[95vw] sm:w-[700px] max-h-[80vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
            // header, content unchanged
        }
    }
}
```

Remove `onclick: stop_propagation()` from the inner card.

Remove the `onclick` on the backdrop div (was: `onclick: move |_| on_close.call(())` — now handled by AnimatedOverlay).

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/context_panel.rs
git commit -m "refactor(ui): migrate ContextDialog to AnimatedOverlay"
```

---

### Task 6: Migrate SkillDetailDialog

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

Has `SkillDialogState { open: bool, skill: Option<...> }` signal.

- [ ] **Step 1: Wrap in AnimatedOverlay**

Replace the outer backdrop div (lines 31-33, 37-39):

```rust
use super::animated_overlay::AnimatedOverlay;

rsx! {
    AnimatedOverlay {
        open,
        on_close: move |_| {
            let mut s = signal.write_unchecked();
            s.open = false;
            s.skill = None;
        },
        div {
            class: "w-[95vw] sm:w-[700px] max-h-[80vh] sm:max-h-[80vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
            // header, content unchanged
        }
    }
}
```

Remove the backdrop `onclick` and the inner card `onclick: stop_propagation()`.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs
git commit -m "refactor(ui): migrate SkillDetailDialog to AnimatedOverlay"
```

---

### Task 7: Migrate conversation ToolDetailModal

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

Private modal rendered inside `ConversationView`. State is `use_signal(|| None::<ToolDetail>)`.

- [ ] **Step 1: Find and wrap the ToolDetailModal**

The modal is rendered around line 521-523. Replace the outer backdrop:

```rust
use super::animated_overlay::AnimatedOverlay;

// The modal renders when detail_signal.read().is_some()
// Replace the outer div:
AnimatedOverlay {
    open: detail_signal.read().is_some(),
    on_close: move |_| detail_signal.set(None),
    div {
        class: "bg-[#1a1a2e] border border-[#444] rounded-lg w-full max-w-[640px] max-h-[80vh] flex flex-col shadow-2xl",
        // header, content unchanged
    }
}
```

Remove backdrop `onclick` and inner card `onclick: stop_propagation()`.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "refactor(ui): migrate conversation ToolDetailModal to AnimatedOverlay"
```

---

### Task 8: Migrate ApprovalDialog

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/approval_dialog.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

Uses `ApprovalUiState` context signal. Has `has_pending()` guard. No backdrop-close (approval requires explicit Approve/Reject).

- [ ] **Step 1: Wrap in AnimatedOverlay**

Replace the outer backdrop div (lines 28-29):

```rust
use super::animated_overlay::AnimatedOverlay;

rsx! {
    AnimatedOverlay {
        open: has_pending,
        on_close: move |_| {},  // No-op: approval requires explicit action
        div {
            class: "bg-[#252540] border border-[#444466] rounded-lg p-3 sm:p-4 w-[95vw] max-w-[600px] sm:min-w-[400px] sm:w-[90vw] sm:max-w-[500px] max-h-[80vh] overflow-y-auto",
            // header, buttons unchanged
        }
    }
}
```

Remove `onclick: stop_propagation()` from the inner card.

Remove the backdrop `onclick` — ApprovalDialog has no backdrop dismiss.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/approval_dialog.rs
git commit -m "refactor(ui): migrate ApprovalDialog to AnimatedOverlay"
```

---

### Task 9: Migrate DebugPanel

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/debug_panel.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

Uses `DebugState` context signal with `open: bool`.

- [ ] **Step 1: Wrap in AnimatedOverlay**

Replace the outer div (line 50):

```rust
use super::animated_overlay::AnimatedOverlay;

rsx! {
    AnimatedOverlay {
        open,
        on_close: move |_| { debug.write_unchecked().open = false; },
        div {
            class: "bg-[#1a1a2e] border border-[#444] rounded-lg flex flex-col shadow-2xl",
            style: "width: 80vw; height: 80vh;",
            // header, content unchanged
        }
    }
}
```

Remove the backdrop `onclick` (wasn't present — DebugPanel closes via × only).

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/debug_panel.rs
git commit -m "refactor(ui): migrate DebugPanel to AnimatedOverlay"
```

---

### Task 10: Migrate TaskDepGraph

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/task_dep_graph.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

Rendered inside `tasks_panel.rs`. Has `on_close: EventHandler<()>` already.

- [ ] **Step 1: Wrap in AnimatedOverlay**

Replace the outer backdrop div (lines 218-219):

```rust
use super::animated_overlay::AnimatedOverlay;

rsx! {
    AnimatedOverlay {
        open: graph_target.read().is_some(),
        on_close: move |_| on_close.call(()),
        div {
            class: "bg-[#252540] border border-[#444466] rounded-lg p-3 sm:p-4 w-[95vw] max-w-[900px] max-h-[85vh] flex flex-col overflow-hidden",
            // header, SVG content unchanged
        }
    }
}
```

Remove backdrop `onclick` and inner card `onclick: stop_propagation()`.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/task_dep_graph.rs
git commit -m "refactor(ui): migrate TaskDepGraph to AnimatedOverlay"
```

---

### Task 11: Migrate SessionDetailOverlay

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

**Interfaces:**
- Consumes: `AnimatedOverlay` from Task 2

Private overlay inside `SessionsPanel`. Uses local `show_detail: Signal<bool>`.

- [ ] **Step 1: Wrap in AnimatedOverlay**

Replace the outer backdrop div (lines 275-279):

```rust
use super::animated_overlay::AnimatedOverlay;

rsx! {
    AnimatedOverlay {
        open: show_detail(),
        on_close: move |_| show_detail.set(false),
        div {
            class: "bg-[#1a1a2e] border border-[#333355] rounded-lg w-[80vw] max-w-[900px] h-[70vh] flex flex-col overflow-hidden",
            // header, entries unchanged
        }
    }
}
```

Remove backdrop `onclick` and inner card `onclick: stop_propagation()`.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/sessions_panel.rs
git commit -m "refactor(ui): migrate SessionDetailOverlay to AnimatedOverlay"
```

---

### Task 12: CapabilityDrawer slide animation

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/capability_drawer.rs`
- Modify: `crates/vol-llm-ui/assets/input.css`

**Interfaces:**
- Consumes: nothing from earlier tasks (independent component)
- Produces: CSS classes `animate-drawer-in`, `animate-drawer-out`

- [ ] **Step 1: Add drawer slide keyframes to input.css**

```css
@keyframes drawer-slide-in {
  from { transform: translateX(100%); }
  to   { transform: translateX(0); }
}
@keyframes drawer-slide-out {
  from { transform: translateX(0); }
  to   { transform: translateX(100%); }
}
```

Add to `@theme`:
```css
--animate-drawer-in:  drawer-slide-in  200ms ease-out both;
--animate-drawer-out: drawer-slide-out 150ms ease-in  both;
```

- [ ] **Step 2: Add state machine to CapabilityDrawer**

Add `OverlayPhase` state machine (same pattern as AnimatedOverlay but local to the drawer):

```rust
use gloo_timers::future::TimeoutFuture;

// At top of CapabilityDrawer component:
let mut phase: Signal<OverlayPhase> = use_signal(|| OverlayPhase::Hidden);

let ds = drawer_state.read();
let open = ds.open;
drop(ds);

// Effect to react to open changes
use_effect(move || {
    let ds = drawer_state.read();
    let open = ds.open;
    drop(ds);
    let next = next_phase(*phase.read(), open);
    if next == *phase.read() { return; }
    phase.set(next);
    match next {
        OverlayPhase::Entering => {
            let mut ph = phase;
            wasm_bindgen_futures::spawn_local(async move {
                TimeoutFuture::new(200).await;
                ph.with_mut(|p| { if *p == OverlayPhase::Entering { *p = OverlayPhase::Visible; } });
            });
        }
        OverlayPhase::Exiting => {
            let mut ph = phase;
            wasm_bindgen_futures::spawn_local(async move {
                TimeoutFuture::new(150).await;
                ph.set(OverlayPhase::Hidden);
            });
        }
        _ => {}
    }
});
```

- [ ] **Step 3: Apply animation classes to backdrop and drawer panel**

Backdrop div (line 127): add conditional `animate-overlay-in` / `animate-overlay-out` based on phase.
Drawer panel div (line 138): add conditional `animate-drawer-in` / `animate-drawer-out` based on phase.
Guard rendering: `if *phase.read() == OverlayPhase::Hidden { return rsx! {}; }`

- [ ] **Step 4: Rebuild CSS**

Run: `make web-css`
Expected: no errors

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/capability_drawer.rs \
        crates/vol-llm-ui/assets/input.css crates/vol-llm-ui/assets/tailwind.css
git commit -m "feat(ui): add slide animation to CapabilityDrawer"
```

---

### Task 13: Clean up dead code

**Files:**
- Delete: `crates/vol-llm-ui/src/web/components/session_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`
- Modify: `crates/vol-llm-ui/src/state/mod.rs` (only if `SessionDialogState` is unreferenced)

- [ ] **Step 1: Remove session_dialog module and re-exports**

In `crates/vol-llm-ui/src/web/components/mod.rs`:
- Remove `pub mod session_dialog;`
- Remove `pub use session_dialog::SessionDialog;`

- [ ] **Step 2: Delete the file**

```bash
rm crates/vol-llm-ui/src/web/components/session_dialog.rs
```

- [ ] **Step 3: Check if SessionDialogState is used elsewhere**

Run: `grep -rn "SessionDialogState" crates/vol-llm-ui/src/`
Expected: only found in `state/mod.rs` definition. If so, remove the struct.

If `SessionDialogState` is in `state/mod.rs`, remove it:
```rust
// Remove the struct definition and impl block for SessionDialogState
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-ui`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mod.rs \
        crates/vol-llm-ui/src/state/mod.rs
git rm crates/vol-llm-ui/src/web/components/session_dialog.rs
git commit -m "chore(ui): remove dead SessionDialog component and state"
```

---

### Task 14: End-to-end verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test -p vol-llm-ui
```
Expected: all tests pass

- [ ] **Step 2: Run coverage check**

```bash
make coverage-threshold PKG=vol-llm-ui PCT=80
```
Expected: coverage ≥ 80%

- [ ] **Step 3: Run web build**

```bash
make web-build
```
Expected: successful WASM build

- [ ] **Step 4: Visual verification checklist**

Start dev servers (`make web-css`, `make web-dev`, `make web-backend`) and verify:

- [ ] Open each dialog type — confirm scale+fade enter animation plays (~200ms)
- [ ] Close each dialog via × button — confirm scale+fade exit animation plays (~150ms)
- [ ] Click backdrop to close — confirm exit animation plays
- [ ] Open CapabilityDrawer — confirm slide-in from right
- [ ] Close CapabilityDrawer — confirm slide-out to right
- [ ] Rapid open/close — no stale state or visual glitches
- [ ] Multiple dialogs in sequence — no z-index stacking issues
- [ ] Mobile viewport (< 480px) — dialogs still centered with padding

- [ ] **Step 5: Commit any final fixes**

Only if issues found during verification.

---

## Known Omissions

- **FileTree mobile drawer**: Uses `absolute` positioning scoped within the main layout (not `fixed` overlay). Its animation would need different treatment — the drawer content is always in DOM with an `open` class toggle, so CSS transitions could work directly without a state machine. Left as a follow-up.
- **NodesDropdown**: Menu/popover pattern, not a modal. Its show/hide is typically hover/focus-driven. Different animation semantics — left as follow-up.
- **z-index stacking for simultaneous dialogs**: All normal dialogs share `z-40`/`z-50`. If two dialogs need to stack (e.g., HITL approval on top of a tool dialog), `AnimatedOverlay` can gain an optional `z_index` prop later.
