//! Session management dialog (list, resume, new, delete).

use dioxus::prelude::*;

use crate::state::SessionDialogEntry;
use crate::web::components::app::AppState;

/// Modal dialog for managing saved sessions.
///
/// Displays when `session_dialog_open` is true.
#[component]
pub fn SessionDialog() -> Element {
    let state: AppState = use_context();
    let open = state.ui_state.peek().session_dialog_open;
    if !open {
        return rsx! {};
    }

    let sessions = state.ui_state.peek().session_dialog_sessions.clone();
    let selected = state.ui_state.peek().session_dialog_selected;
    let current_session = state.ui_state.peek().session_id.clone();

    let on_close = move |_: Event<MouseData>| {
        state.ui_state.write_silent().session_dialog_open = false;
    };

    let on_new = move |_: Event<MouseData>| {
        let new_id = uuid_v4_stub();
        let mut s = state.ui_state.write_silent();
        s.session_id = new_id;
        s.session_dialog_open = false;
    };

    let on_resume = move |_: Event<MouseData>| {
        let mut s = state.ui_state.write_silent();
        let selected = s.session_dialog_selected;
        let session_id = s
            .session_dialog_sessions
            .get(selected)
            .map(|s| s.session_id.clone())
            .unwrap_or_default();
        if !session_id.is_empty() {
            s.session_id = session_id;
        }
        s.session_dialog_open = false;
    };

    let on_delete = move |_: Event<MouseData>| {
        let mut s = state.ui_state.write_silent();
        let selected = s.session_dialog_selected;
        let current = s.session_id.clone();
        if let Some(entry) = s.session_dialog_sessions.get(selected) {
            if entry.session_id != current {
                s.session_dialog_sessions.remove(selected);
                if !s.session_dialog_sessions.is_empty() {
                    s.session_dialog_selected = selected
                        .min(s.session_dialog_sessions.len().saturating_sub(1));
                }
            }
        }
    };

    let session_items: Vec<Element> = if sessions.is_empty() {
        vec![rsx! {
            div { class: "modal-empty", "No saved sessions found." }
        }]
    } else {
        sessions
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let is_selected = i == selected;
                let cls = if is_selected {
                    "modal-session-item selected"
                } else {
                    "modal-session-item"
                };
                let short_id = if entry.session_id.len() > 10 {
                    format!("{}...", &entry.session_id[..7])
                } else {
                    entry.session_id.clone()
                };
                let idx = i;
                rsx! {
                    div {
                        class: cls,
                        onclick: move |_: Event<MouseData>| {
                            state.ui_state.write_silent().session_dialog_selected = idx;
                        },
                        span { class: "modal-session-id", "{short_id}" }
                        span { class: "modal-session-meta",
                            "{entry.entry_count} entries | {entry.age_label}"
                        }
                    }
                }
            })
            .collect()
    };

    rsx! {
        div { class: "modal-overlay", onclick: on_close,
            div { class: "modal-content", onclick: |evt: Event<MouseData>| {
                evt.stop_propagation();
            },
                div { class: "modal-title", "Sessions" }
                {session_items.into_iter()}
                div { class: "modal-actions",
                    button { class: "btn-new", onclick: on_new, "New" }
                    button { class: "btn-resume", onclick: on_resume, "Resume" }
                    button { class: "btn-delete", onclick: on_delete, "Delete" }
                    button { class: "btn-cancel", onclick: on_close, "Cancel" }
                }
            }
        }
    }
}

/// Simple UUID v4 stub for web builds (avoids UUID dependency in WASM).
fn uuid_v4_stub() -> String {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (ts >> 96) as u32,
        (ts >> 80) as u16 & 0xffff,
        (ts >> 64) as u16 & 0xffff,
        ((ts >> 48) as u16 & 0x0fff) | 0x4000,
        ts & 0xffffffffffff
    )
}
