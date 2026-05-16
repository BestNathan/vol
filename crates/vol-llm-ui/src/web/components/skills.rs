//! Skills panel showing available skills.

use dioxus::prelude::*;
use crate::state::{SkillDialogState, SkillsState};
use crate::web::components::app::AppState;

#[component]
pub fn SkillsPanel(mut dialog_signal: Signal<SkillDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();
    let rpc_client_for_effect = rpc_client.clone();

    let signal = use_signal(|| SkillsState::new());

    // Fetch skills on mount
    use_effect(move || {
        let client = rpc_client_for_effect.clone();
        let sig = signal.clone();
        client.skill_list(move |result| {
            match result {
                Ok(entries) => {
                    sig.write_unchecked().skills = entries.iter().map(|e| {
                        crate::state::SkillDisplayEntry {
                            name: e.name.clone(),
                            version: e.version.clone(),
                            scope: e.scope.clone(),
                            description: e.description.clone(),
                        }
                    }).collect();
                    sig.write_unchecked().error = None;
                }
                Err(e) => {
                    sig.write_unchecked().error = Some(e);
                }
            }
        });
    });

    let count = signal.read().skills.len();
    let error = signal.read().error.clone();

    if let Some(_err) = error {
        let retry_client = rpc_client.clone();
        let retry_sig = signal.clone();
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2.5",
                div { class: "flex flex-col items-center justify-center h-full text-[#c04040]",
                    "Failed to load skills"
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[12px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let client = retry_client.clone();
                            let sig = retry_sig.clone();
                            client.skill_list(move |result| {
                                match result {
                                    Ok(entries) => {
                                        sig.write_unchecked().skills = entries.iter().map(|e| {
                                            crate::state::SkillDisplayEntry {
                                                name: e.name.clone(),
                                                version: e.version.clone(),
                                                scope: e.scope.clone(),
                                                description: e.description.clone(),
                                            }
                                        }).collect();
                                        sig.write_unchecked().error = None;
                                    }
                                    Err(e) => { sig.write_unchecked().error = Some(e); }
                                }
                            });
                        },
                        "Retry"
                    }
                }
            }
        };
    }

    if count == 0 {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No skills discovered" } } };
    }
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            table { class: "skills-table",
                thead { tr {
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Name" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Version" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Scope" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Description" }
                } }
                tbody {
                    {(0..count).map(|i| { let s = signal.clone(); let d = dialog_signal.clone(); let c = rpc_client.clone(); rsx! { SkillRow { signal: s, dialog_signal: d, rpc_client: c, index: i } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
}

#[component]
fn SkillRow(
    signal: Signal<SkillsState>,
    mut dialog_signal: Signal<SkillDialogState>,
    rpc_client: crate::web::client::JsonRpcClient,
    index: usize,
) -> Element {
    let skill = signal.read().skills.get(index).cloned();
    let Some(skill) = skill else { return rsx! {}; };

    let color = match skill.scope.as_str() { "User" => "#40c040", "Repo" => "#4080ff", _ => "#c0c040" };

    rsx! {
        tr {
            class: "cursor-pointer hover:bg-[#2a2a44]",
            onclick: move |_| {
                let client = rpc_client.clone();
                let name = skill.name.clone();
                let mut d = dialog_signal.write_unchecked();
                d.open = true;
                d.skill = None;
                d.loading = true;
                client.skill_get(&name, move |result| {
                    match result {
                        Ok(detail) => {
                            d.skill = Some(detail);
                        }
                        Err(_) => {
                            d.skill = None;
                        }
                    }
                    d.loading = false;
                });
            },
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#e0e0e0] font-bold", "{skill.name}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.version}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44]", style: "color: {color};", "{skill.scope}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.description}" }
        }
    }
}
