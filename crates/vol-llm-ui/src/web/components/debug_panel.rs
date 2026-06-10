//! Debug panel — WS message inspector and development tools.

use crate::state::{DebugState, DebugTab, WsDirection};
use dioxus::prelude::*;

fn format_elapsed(ms: u64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    let hours = mins / 60;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        hours,
        mins % 60,
        secs % 60,
        ms % 1000
    )
}

fn format_json_pretty(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    }
}

fn tab_label(tab: DebugTab) -> &'static str {
    match tab {
        DebugTab::Ws => "WS",
    }
}

#[component]
pub fn DebugPanel() -> Element {
    let debug = use_context::<Signal<DebugState>>();
    let guard = debug.read();
    let messages = guard.ws_messages.clone();
    let open = guard.open;
    let active_tab = guard.active_tab;
    drop(guard);

    if !open {
        return rsx! { div {} };
    }

    rsx! {
        div { class: "fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4",
            div { class: "bg-[#1a1a2e] border border-[#444] rounded-lg flex flex-col shadow-2xl",
                style: "width: 80vw; height: 80vh;",
                div { class: "flex items-center justify-between px-4 py-2 border-b border-[#333] shrink-0",
                    div { class: "flex items-center gap-3",
                        span { class: "text-[#e0e0e0] font-bold text-sm", "Debug Panel" }
                        div { class: "flex gap-1",
                            {
                                let tab = DebugTab::Ws;
                                let cls = if tab == active_tab {
                                    "px-3 py-1 text-[12px] font-semibold cursor-pointer border-b-2 border-[#80a0ff] text-[#e0e0e0]"
                                } else {
                                    "px-3 py-1 text-[12px] cursor-pointer text-[#888] hover:text-[#ccc] border-b-2 border-transparent"
                                };
                                rsx! {
                                    button {
                                        class: "{cls}",
                                        onclick: move |_| { debug.write_unchecked().active_tab = tab; },
                                        {tab_label(tab)}
                                    }
                                }
                            }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-white text-lg leading-none px-1",
                        onclick: {
                            let d = debug;
                            move |_| { d.write_unchecked().open = false; }
                        },
                        "×"
                    }
                }
                div { class: "flex-1 overflow-hidden",
                    match active_tab {
                        DebugTab::Ws => rsx! { WsTab { messages } },
                    }
                }
            }
        }
    }
}

#[component]
fn WsTab(messages: Vec<crate::state::WsMessage>) -> Element {
    let expanded = use_signal(|| None::<usize>);

    rsx! {
        div { class: "flex flex-col h-full",
            div { class: "flex-1 overflow-y-auto font-mono text-xs",
                if messages.is_empty() {
                    div { class: "flex items-center justify-center h-full text-[#666] text-sm",
                        "No messages yet. Open the panel while the agent is active to capture WS traffic."
                    }
                } else {
                    for (i, msg) in messages.iter().enumerate() {
                        WsMessageRow { index: i, message: msg.clone(), expanded }
                    }
                }
            }
            div { class: "px-3 py-1.5 border-t border-[#333] text-[10px] text-[#666] shrink-0 flex items-center justify-between",
                span { "{messages.len()} messages" }
                span { "Recording since page load" }
            }
        }
    }
}

#[component]
fn WsMessageRow(
    index: usize,
    message: crate::state::WsMessage,
    expanded: Signal<Option<usize>>,
) -> Element {
    let is_expanded = *expanded.read() == Some(index);
    let arrow = match message.direction {
        WsDirection::In => "\u{2190}",
        WsDirection::Out => "\u{2192}",
    };
    let arrow_color = match message.direction {
        WsDirection::In => "#40c040",
        WsDirection::Out => "#80a0ff",
    };
    let stamp = format_elapsed(message.elapsed_ms);

    rsx! {
        div {
            class: "border-b border-[#222] hover:bg-[#222240] cursor-pointer",
            onclick: {
                let mut e = expanded;
                move |_| { e.with_mut(|s| if *s == Some(index) { *s = None } else { *s = Some(index) }); }
            },
            div { class: "flex items-center gap-2 px-3 py-1.5",
                span { class: "text-[#555] w-[100px] shrink-0", "{stamp}" }
                span { style: "color: {arrow_color}; font-weight: bold;", "{arrow}" }
                span { class: "text-[#ccc] truncate", "{message.method}" }
            }
            if is_expanded {
                div { class: "px-3 pb-2 pl-[120px]",
                    pre { class: "text-[#888] text-[11px] bg-[#111128] rounded p-2 whitespace-pre-wrap break-all max-h-[300px] overflow-y-auto",
                        "{format_json_pretty(&message.payload)}"
                    }
                }
            }
        }
    }
}
