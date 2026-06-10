//! Dialog showing full details of a skill with file viewer.

use crate::state::SkillDialogState;
use crate::web::components::app::AppState;
use dioxus::prelude::*;

#[component]
pub fn SkillDetailDialog(mut signal: Signal<SkillDialogState>) -> Element {
    let (open, skill) = {
        let s = signal.read();
        (s.open, s.skill.clone())
    };

    let mut selected_file: Signal<Option<String>> = use_signal(|| None);
    let mut file_content: Signal<Option<(String, String, bool)>> = use_signal(|| None);

    // Reset internal signals when skill changes
    use_effect(move || {
        selected_file.set(None);
        file_content.set(None);
    });

    if !open {
        return rsx! {};
    }

    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            onclick: move |_| {
                let mut s = signal.write_unchecked();
                s.open = false;
                s.skill = None;
            },
            div {
                class: "w-[95vw] sm:w-[700px] max-h-[80vh] sm:max-h-[80vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
                onclick: move |evt: Event<MouseData>| { evt.stop_propagation(); },
                // Header
                div { class: "flex items-center justify-between flex-shrink-0 px-4 pt-3 pb-2 border-b border-[#3a3a55]",
                    div { class: "flex items-center gap-2 min-w-0",
                        if let Some(ref s) = skill {
                            span { class: "text-[15px] font-semibold text-[#e0e0e0] truncate", "{s.name}" }
                            span { class: "text-[11px] text-[#888] bg-[#2a2a44] px-1.5 py-0.5 rounded flex-shrink-0", "v{s.version}" }
                            span {
                                class: "text-[11px] px-1.5 py-0.5 rounded flex-shrink-0",
                                style: {
                                    let color = if s.scope == "User" { "#40c040" } else { "#4080ff" };
                                    format!("color: {color}; background: #2a2a44;")
                                },
                                "{s.scope}"
                            }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px] flex-shrink-0 ml-2",
                        onclick: move |_| {
                            let mut s = signal.write_unchecked();
                            s.open = false;
                            s.skill = None;
                        },
                        "x"
                    }
                }
                // Scrollable content area
                div { class: "flex-1 min-h-0 overflow-y-auto px-4 pb-4",
                    if let Some(ref detail) = skill {
                        // Description
                        div { class: "text-[#ccc] text-[13px] mb-2 mt-2 break-words", "{detail.description}" }

                        // Triggers
                        if !detail.triggers.is_empty() {
                            div { class: "flex gap-1.5 flex-wrap mb-3",
                                {detail.triggers.iter().enumerate().map(|(i, t)| {
                                    let t = t.clone();
                                    rsx! {
                                        span { key: "{i}", class: "text-[11px] text-[#c0c040] bg-[#2a2a20] px-2 py-0.5 rounded", "{t}" }
                                    }
                                }).collect::<Vec<Element>>().into_iter()}
                            }
                        }

                        // SKILL.md body
                        div { class: "bg-[#12121e] border border-[#2a2a44] rounded p-2 mb-3 max-h-[200px] overflow-y-auto",
                            pre { class: "text-[12px] text-[#aaa] font-mono whitespace-pre-wrap", "{detail.content}" }
                        }

                        // File listing + preview
                        if !detail.file_listing.is_empty() {
                            div { class: "flex flex-col",
                                div { class: "text-[11px] text-[#888] mb-1 font-semibold", "Files" }
                                div { class: "bg-[#12121e] border border-[#2a2a44] rounded max-h-[150px] overflow-y-auto mb-2",
                                    {detail.file_listing.iter().enumerate().map(|(i, f)| {
                                        let f = f.clone();
                                        let dir = detail.directory.clone();
                                        let sel = selected_file.clone();
                                        let fc = file_content.clone();
                                        let client = rpc_client.clone();
                                        let name = f.split('/').last().unwrap_or(&f).to_string();
                                        let is_selected = selected_file.read().as_ref() == Some(&f);
                                        let row_bg = if is_selected { "#2a3a4a" } else { "transparent" };
                                        let abs_path = if dir.is_empty() {
                                            f.clone()
                                        } else {
                                            format!("{dir}/{f}")
                                        };
                                        rsx! {
                                            div {
                                                key: "{i}",
                                                class: "text-[12px] text-[#aaa] font-mono px-2 py-0.5 border-b border-[#2a2a44] last:border-b-0 cursor-pointer hover:bg-[#2a2a44]",
                                                style: "background-color: {row_bg};",
                                                onclick: move |_| {
                                                    let mut sel = sel.clone();
                                                    let mut fc = fc.clone();
                                                    let client = client.clone();
                                                    let full_path = abs_path.clone();
                                                    sel.set(Some(f.clone()));
                                                    fc.set(Some((f.clone(), String::new(), true)));
                                                    let path_for_closure = f.clone();
                                                    let mut sig = fc.clone();
                                                    client.file_read(&full_path, move |result| {
                                                        match result {
                                                            Ok(content) => {
                                                                sig.set(Some((path_for_closure.clone(), content, false)));
                                                            }
                                                            Err(e) => {
                                                                sig.set(Some((path_for_closure, format!("Error: {e}"), false)));
                                                            }
                                                        }
                                                    });
                                                },
                                                "{name}"
                                            }
                                        }
                                    }).collect::<Vec<Element>>().into_iter()}
                                }
                                // Content preview
                                div { class: "border border-[#2a2a44] rounded min-h-[100px] max-h-[250px] overflow-y-auto p-2",
                                    match file_content.read().as_ref() {
                                        Some((path, _content, true)) => {
                                            rsx! {
                                                div { class: "flex items-center gap-2 text-[#888] text-[13px]",
                                                    div { class: "text-[11px] text-[#666] font-mono", "{path}" }
                                                    "Loading..."
                                                }
                                            }
                                        }
                                        Some((_path, content, false)) => {
                                            rsx! {
                                                pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-words",
                                                    "{content}"
                                                }
                                            }
                                        }
                                        None => {
                                            rsx! {
                                                div { class: "text-[#666] text-[13px] text-center py-8", "Click a file to preview" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        div { class: "text-[#c04040] text-[13px] py-4 text-center", "Failed to load skill details" }
                    }
                }
            }
        }
    }
}
