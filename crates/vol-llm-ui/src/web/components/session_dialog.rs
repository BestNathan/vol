//! Session management dialog (list, resume, new, delete).

use dioxus::prelude::*;
use crate::state::SessionDialogState;

fn uuid_v4_stub() -> String {
    let ts = js_sys::Date::now() as u128 * 1_000_000;
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (ts >> 96) as u32, (ts >> 80) as u16 & 0xffff, (ts >> 64) as u16 & 0xffff,
        ((ts >> 48) as u16 & 0x0fff) | 0x4000, ts & 0xffffffffffff)
}

#[component]
pub fn SessionDialog() -> Element {
    let signal = use_signal(|| SessionDialogState::new());
    let open = signal.read().open;
    if !open { return rsx! {}; }

    let sessions = signal.read().sessions.clone();
    let selected = signal.read().selected;

    let mut sig_new = signal;
    let on_new = move |_: Event<MouseData>| { sig_new.with_mut(|s| { s.open = false; let _ = uuid_v4_stub(); }); };
    let mut sig_resume = signal;
    let on_resume = move |_: Event<MouseData>| { sig_resume.with_mut(|s| s.open = false); };
    let mut sig_delete = signal;
    let on_delete = move |_: Event<MouseData>| {
        sig_delete.with_mut(|s| {
            let sel = s.selected;
            if s.sessions.get(sel).is_some() {
                s.sessions.remove(sel);
                if !s.sessions.is_empty() { s.selected = sel.min(s.sessions.len().saturating_sub(1)); }
            }
        });
    };

    let items: Vec<Element> = if sessions.is_empty() {
        vec![rsx! { div { class: "modal-empty", "No saved sessions found." } }]
    } else {
        sessions.iter().enumerate().map(|(i, entry)| {
            let is_sel = i == selected;
            let cls = if is_sel { "modal-session-item selected" } else { "modal-session-item" };
            let short = if entry.session_id.len() > 10 { format!("{}...", &entry.session_id[..7]) } else { entry.session_id.clone() };
            let mut sig_sel = signal;
            rsx! {
                div { class: cls, onclick: move |_: Event<MouseData>| { sig_sel.with_mut(|s| s.selected = i); },
                    span { class: "modal-session-id", "{short}" }
                    span { class: "modal-session-meta", "{entry.entry_count} entries | {entry.age_label}" }
                }
            }
        }).collect()
    };

    let mut sig_overlay = signal;
    let mut sig_cancel = signal;
    rsx! {
        div { class: "modal-overlay", onclick: move |_: Event<MouseData>| { sig_overlay.with_mut(|s| s.open = false); },
            div { class: "modal-content", onclick: |evt: Event<MouseData>| { evt.stop_propagation(); },
                div { class: "modal-title", "Sessions" }
                {items.into_iter()}
                div { class: "modal-actions",
                    button { class: "btn-new", onclick: on_new, "New" }
                    button { class: "btn-resume", onclick: on_resume, "Resume" }
                    button { class: "btn-delete", onclick: on_delete, "Delete" }
                    button { class: "btn-cancel", onclick: move |_: Event<MouseData>| { sig_cancel.with_mut(|s| s.open = false); }, "Cancel" }
                }
            }
        }
    }
}
