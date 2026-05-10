//! Session management dialog (list, resume, new, delete).

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Modal dialog for managing saved sessions.
#[component]
pub fn SessionDialog() -> Element {
    let state: AppState = use_context();
    let open = state.signal.read().session_dialog_open;
    if !open {
        return rsx! {};
    }

    let sessions = state.signal.read().session_dialog_sessions.clone();
    let selected = state.signal.read().session_dialog_selected;

    let mut sig_new = state.signal;
    let on_new = move |_: Event<MouseData>| {
        let new_id = uuid_v4_stub();
        sig_new.with_mut(|s| {
            s.session_id = new_id;
            s.session_dialog_open = false;
        });
    };

    let mut sig_resume = state.signal;
    let on_resume = move |_: Event<MouseData>| {
        sig_resume.with_mut(|s| {
            let sel = s.session_dialog_selected;
            let session_id = s.session_dialog_sessions.get(sel).map(|s| s.session_id.clone()).unwrap_or_default();
            if !session_id.is_empty() {
                s.session_id = session_id;
            }
            s.session_dialog_open = false;
        });
    };

    let mut sig_delete = state.signal;
    let on_delete = move |_: Event<MouseData>| {
        sig_delete.with_mut(|s| {
            let sel = s.session_dialog_selected;
            let current = s.session_id.clone();
            if let Some(entry) = s.session_dialog_sessions.get(sel) {
                if entry.session_id != current {
                    s.session_dialog_sessions.remove(sel);
                    if !s.session_dialog_sessions.is_empty() {
                        s.session_dialog_selected = sel.min(s.session_dialog_sessions.len().saturating_sub(1));
                    }
                }
            }
        });
    };

    let session_items: Vec<Element> = if sessions.is_empty() {
        vec![rsx! {
            div { class: "modal-empty", "No saved sessions found." }
        }]
    } else {
        sessions.iter().enumerate().map(|(i, entry)| {
            let is_selected = i == selected;
            let cls = if is_selected { "modal-session-item selected" } else { "modal-session-item" };
            let short_id = if entry.session_id.len() > 10 {
                format!("{}...", &entry.session_id[..7])
            } else {
                entry.session_id.clone()
            };
            let mut sig_sel = state.signal;
            let idx = i;
            rsx! {
                div {
                    class: cls,
                    onclick: move |_: Event<MouseData>| {
                        sig_sel.with_mut(|s| {
                            s.session_dialog_selected = idx;
                        });
                    },
                    span { class: "modal-session-id", "{short_id}" }
                    span { class: "modal-session-meta",
                        "{entry.entry_count} entries | {entry.age_label}"
                    }
                }
            }
        }).collect()
    };

    let mut sig_overlay = state.signal;
    let mut sig_cancel = state.signal;
    rsx! {
        div { class: "modal-overlay", onclick: move |_: Event<MouseData>| {
            sig_overlay.with_mut(|s| {
                s.session_dialog_open = false;
            });
        },
            div { class: "modal-content", onclick: |evt: Event<MouseData>| {
                evt.stop_propagation();
            },
                div { class: "modal-title", "Sessions" }
                {session_items.into_iter()}
                div { class: "modal-actions",
                    button { class: "btn-new", onclick: on_new, "New" }
                    button { class: "btn-resume", onclick: on_resume, "Resume" }
                    button { class: "btn-delete", onclick: on_delete, "Delete" }
                    button { class: "btn-cancel", onclick: move |_: Event<MouseData>| {
                        sig_cancel.with_mut(|s| {
                            s.session_dialog_open = false;
                        });
                    }, "Cancel" }
                }
            }
        }
    }
}

/// Simple UUID v4 stub for web builds.
fn uuid_v4_stub() -> String {
    let ts = js_sys::Date::now() as u128;
    let ts = ts * 1_000_000;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (ts >> 96) as u32,
        (ts >> 80) as u16 & 0xffff,
        (ts >> 64) as u16 & 0xffff,
        ((ts >> 48) as u16 & 0x0fff) | 0x4000,
        ts & 0xffffffffffff
    )
}
