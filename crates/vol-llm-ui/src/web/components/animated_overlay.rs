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
    // Last `open` value the effect acted on. The effect also re-runs on
    // `phase` changes (it reads `phase`), so this guard makes sure it only
    // reacts to real `open` transitions — otherwise a backdrop-click close
    // (which sets `phase = Exiting` while `open` is still true) would be
    // immediately overwritten by `next_phase(Exiting, true) = Entering`,
    // replacing the exit animation with a re-enter after ~1 frame.
    // Initialized to `!open` so a dialog that mounts already-open still
    // runs the enter animation on its first effect run.
    let mut prev_open = use_signal(|| !open);

    // React to `open` prop changes: kick off enter or exit animation.
    // NOTE: `open` is a plain (non-signal) prop, so we bridge it into the
    // reactive world with `use_reactive` — otherwise the effect would only
    // re-run on `phase` changes and the overlay would never respond to
    // external `open` toggles (Dioxus 0.6 does not track plain prop reads).
    use_effect(use_reactive((&open,), move |(open,)| {
        // No change in the `open` prop — this run was triggered by a phase
        // change (or the prev_open bookkeeping write), not by an external
        // open/close. Leave the phase machine alone so an in-flight exit
        // animation is not replaced by a re-enter.
        if open == *prev_open.read() {
            return;
        }
        prev_open.set(open);

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
                    ph.with_mut(|p| {
                        // Only finish hiding if still exiting — a re-open
                        // during the exit animation restarts Entering and
                        // must not be cut short.
                        if *p == OverlayPhase::Exiting {
                            *p = OverlayPhase::Hidden;
                        }
                    });
                });
            }
            _ => {}
        }
    }));

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
                        ph.with_mut(|p| {
                            // Guarded like the effect's exit timer: only
                            // finish hiding if still exiting — a re-open
                            // during the exit animation restarted Entering
                            // and must not be yanked to Hidden.
                            if *p == OverlayPhase::Exiting {
                                *p = OverlayPhase::Hidden;
                            }
                        });
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
