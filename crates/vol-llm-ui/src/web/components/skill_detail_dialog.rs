//! Dialog showing full details of a skill with file viewer.

use dioxus::prelude::*;
use crate::state::SkillDialogState;
use crate::web::components::app::AppState;

#[component]
pub fn SkillDetailDialog(mut signal: Signal<SkillDialogState>) -> Element {
    let (open, skill, loading) = {
        let s = signal.read();
        (s.open, s.skill.clone(), s.loading)
    };

    if !open {
        return rsx! {};
    }

    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();
    let selected_file: Signal<Option<String>> = use_signal(|| None);
    // (file_path, content, is_loading)
    let file_content: Signal<Option<(String, String, bool)>> = use_signal(|| None);

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[800px] h-[80vh] flex flex-col overflow-hidden",
                // Header
                div { class: "flex items-center justify-between mb-2 flex-shrink-0",
                    div { class: "flex items-center gap-2",
                        if let Some(ref s) = skill {
                            span { class: "text-[16px] font-semibold text-[#e0e0e0]", "{s.name}" }
                            span { class: "text-[11px] text-[#888] bg-[#2a2a44] px-1.5 py-0.5 rounded", "v{s.version}" }
                            span {
                                class: "text-[11px] px-1.5 py-0.5 rounded",
                                style: {
                                    let color = if s.scope == "User" { "#40c040" } else { "#4080ff" };
                                    format!("color: {color}; background: #2a2a44;")
                                },
                                "{s.scope}"
                            }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px]",
                        onclick: move |_| {
                            let mut s = signal.write_unchecked();
                            s.open = false;
                            s.skill = None;
                        },
                        "x"
                    }
                }
                if loading {
                    div { class: "text-[#888] text-[13px] py-8 text-center", "Loading skill details..." }
                } else if let Some(ref detail) = skill {
                    // Description
                    div { class: "text-[#ccc] text-[13px] mb-2", "{detail.description}" }

                    // Triggers
                    if !detail.triggers.is_empty() {
                        div { class: "flex gap-1.5 flex-wrap mb-2",
                            {detail.triggers.iter().enumerate().map(|(i, t)| {
                                let t = t.clone();
                                rsx! {
                                    span { key: "{i}", class: "text-[11px] text-[#c0c040] bg-[#2a2a20] px-2 py-0.5 rounded", "{t}" }
                                }
                            }).collect::<Vec<Element>>().into_iter()}
                        }
                    }

                    // SKILL.md body
                    div { class: "bg-[#12121e] border border-[#2a2a44] rounded p-2 max-h-48 overflow-y-auto mb-3",
                        pre { class: "text-[12px] text-[#aaa] font-mono whitespace-pre-wrap", "{detail.content}" }
                    }

                    // File listing + preview
                    if !detail.file_listing.is_empty() {
                        div { class: "flex flex-col",
                            div { class: "text-[11px] text-[#888] mb-1 font-semibold", "Files" }
                            div { class: "bg-[#12121e] border border-[#2a2a44] rounded max-h-[30%] overflow-y-auto mb-2",
                                {detail.file_listing.iter().enumerate().map(|(i, f)| {
                                    let f = f.clone();
                                    let sel = selected_file.clone();
                                    let fc = file_content.clone();
                                    let client = rpc_client.clone();
                                    let name = f.split('/').last().unwrap_or(&f).to_string();
                                    let is_selected = selected_file.read().as_ref() == Some(&f);
                                    let row_bg = if is_selected { "#2a3a4a" } else { "transparent" };
                                    rsx! {
                                        div {
                                            key: "{i}",
                                            class: "text-[12px] text-[#aaa] font-mono px-2 py-0.5 border-b border-[#2a2a44] last:border-b-0 cursor-pointer hover:bg-[#2a2a44]",
                                            style: "background-color: {row_bg};",
                                            onclick: move |_| {
                                                let mut sel = sel.clone();
                                                let mut fc = fc.clone();
                                                let client = client.clone();
                                                sel.set(Some(f.clone()));
                                                fc.set(Some((f.clone(), String::new(), true)));
                                                let path_for_read = f.clone();
                                                let path_for_closure = path_for_read.clone();
                                                let mut sig = fc.clone();
                                                client.file_read(&path_for_read, move |result| {
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
                            div { class: "border border-[#2a2a44] rounded min-h-[120px] p-2",
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
                                            pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-all overflow-auto max-h-[40vh]",
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
