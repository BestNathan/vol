//! Dialog showing full details of a skill.

use dioxus::prelude::*;
use crate::state::SkillDialogState;

#[component]
pub fn SkillDetailDialog(mut signal: Signal<SkillDialogState>) -> Element {
    let (open, skill, loading) = {
        let s = signal.read();
        (s.open, s.skill.clone(), s.loading)
    };

    if !open {
        return rsx! {};
    }

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[650px] max-w-[90vw] max-h-[85vh] flex flex-col",
                // Header
                div { class: "flex items-center justify-between mb-3",
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
                    div { class: "text-[#ccc] text-[13px] mb-3", "{detail.description}" }

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
                    div { class: "bg-[#12121e] border border-[#2a2a44] rounded p-2 max-h-48 overflow-y-auto mb-3",
                        pre { class: "text-[12px] text-[#aaa] font-mono whitespace-pre-wrap", "{detail.content}" }
                    }

                    // File listing
                    if !detail.file_listing.is_empty() {
                        div { class: "text-[11px] text-[#888] mb-1 font-semibold", "Files" }
                        div { class: "bg-[#12121e] border border-[#2a2a44] rounded max-h-32 overflow-y-auto mb-3",
                            {detail.file_listing.iter().enumerate().map(|(i, f)| {
                                let f = f.clone();
                                rsx! {
                                    div { key: "{i}", class: "text-[12px] text-[#aaa] font-mono px-2 py-0.5 border-b border-[#2a2a44] last:border-b-0", "{f}" }
                                }
                            }).collect::<Vec<Element>>().into_iter()}
                        }
                    }
                } else {
                    div { class: "text-[#c04040] text-[13px] py-4 text-center", "Failed to load skill details" }
                }
            }
        }
    }
}
