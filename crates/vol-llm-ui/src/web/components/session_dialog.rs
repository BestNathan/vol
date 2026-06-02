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
        vec![rsx! { div { class: "text-[#888] py-2.5", "No saved sessions found." } }]
    } else {
        sessions.iter().enumerate().map(|(i, entry)| {
            let is_sel = i == selected;
            let cls = if is_sel {
                "px-2 py-1.5 border-b border-[#2a2a44] flex items-center gap-2 bg-[#2a2a44]"
            } else {
                "px-2 py-1.5 border-b border-[#2a2a44] flex items-center gap-2"
            };
            let short = if entry.session_id.len() > 10 { format!("{}...", &entry.session_id[..7]) } else { entry.session_id.clone() };
            let mut sig_sel = signal;
            rsx! {
                div { class: cls, onclick: move |_: Event<MouseData>| { sig_sel.with_mut(|s| s.selected = i); },
                    span { class: "font-mono text-[#e0e0e0] font-bold", "{short}" }
                    span { class: "text-[#888] text-[12px]", "{entry.entry_count} entries | {entry.age_label}" }
                }
            }
        }).collect()
    };

    let mut sig_overlay = signal;
    let mut sig_cancel = signal;
    rsx! {
        div { class: "fixed inset-0 bg-black/60 flex items-center justify-center z-[100]", onclick: move |_: Event<MouseData>| { sig_overlay.with_mut(|s| s.open = false); },
            div { class: "bg-[#252540] border border-[#444466] rounded-lg p-3 sm:p-4 w-[95vw] max-w-[600px] sm:min-w-[400px] sm:w-[90vw] sm:max-w-[500px] max-h-[80vh] overflow-y-auto", onclick: |evt: Event<MouseData>| { evt.stop_propagation(); },
                div { class: "text-[16px] font-bold text-[#e0e0e0] mb-3 border-b border-[#333355] pb-2", "Sessions" }
                {items.into_iter()}
                div { class: "mt-3 flex gap-2 pt-2 border-t border-[#333355]",
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#4060c0] text-[#e0e0e0]", onclick: on_new, "New" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#408040] text-[#e0e0e0]", onclick: on_resume, "Resume" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#804040] text-[#e0e0e0]", onclick: on_delete, "Delete" }
                    button { class: "px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#555] text-[#e0e0e0]", onclick: move |_: Event<MouseData>| { sig_cancel.with_mut(|s| s.open = false); }, "Cancel" }
                }
            }
        }
    }
}
