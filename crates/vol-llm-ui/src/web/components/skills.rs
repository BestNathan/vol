//! Skills panel showing available skills.

use dioxus::prelude::*;
use crate::state::{SkillDialogState, SkillsState, UiEventKind};
use crate::web::components::app::AppState;

/// Safely write to a Signal in an async callback.
/// When a component unmounts, its signals are dropped.
/// Async callbacks may still fire after unmount and panic on `with_mut()`.
fn safe_write<T>(mut sig: Signal<T>, f: impl FnOnce(&mut T)) -> bool {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sig.with_mut(f);
    })).is_ok()
}

#[component]
pub fn SkillsPanel(mut dialog_signal: Signal<SkillDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();

    let signal = use_signal(|| SkillsState::new());

    // Fetch skills on mount
    let rpc_client_for_effect = rpc_client.clone();
    use_effect(move || {
        let client = rpc_client_for_effect.clone();
        let mut sig = signal;
        sig.with_mut(|s| { s.loading = true; s.error = None; });
        client.skill_list(move |result| {
            safe_write(sig, |s| {
                s.loading = false;
                match result {
                    Ok(entries) => {
                        s.skills = entries.iter().map(|e| {
                            crate::state::SkillDisplayEntry {
                                name: e.name.clone(),
                                version: e.version.clone(),
                                scope: e.scope.clone(),
                                description: e.description.clone(),
                            }
                        }).collect();
                        s.error = None;
                    }
                    Err(e) => {
                        s.error = Some(e);
                    }
                }
            });
        });
    });

    // Re-fetch on reconnect
    let event_bus = app_state.event_bus.clone();
    let client_for_reconnect = rpc_client.clone();
    let sig_for_reconnect = signal;
    use_hook(move || {
        let _sub = event_bus.subscribe(UiEventKind::WsConnected, move |_| {
            let cl = client_for_reconnect.clone();
            let sig = sig_for_reconnect;
            safe_write(sig, |s| { s.loading = true; s.error = None; });
            cl.skill_list(move |result| {
                safe_write(sig, |s| {
                    s.loading = false;
                    match result {
                        Ok(entries) => {
                            s.skills = entries.iter().map(|e| {
                                crate::state::SkillDisplayEntry {
                                    name: e.name.clone(),
                                    version: e.version.clone(),
                                    scope: e.scope.clone(),
                                    description: e.description.clone(),
                                }
                            }).collect();
                            s.error = None;
                        }
                        Err(e) => { s.error = Some(e); }
                    }
                });
            });
        });
    });

    let count = signal.read().skills.len();
    let error = signal.read().error.clone();
    let loading = signal.read().loading;

    if let Some(_err) = error {
        let retry_client = rpc_client.clone();
        let retry_sig = signal;
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2.5",
                div { class: "flex flex-col items-center justify-center h-full text-[#c04040]",
                    "Failed to load skills"
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[12px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let client = retry_client.clone();
                            let sig = retry_sig;
                            safe_write(sig, |s| { s.loading = true; s.error = None; });
                            client.skill_list(move |result| {
                                safe_write(sig, |s| {
                                    s.loading = false;
                                    match result {
                                        Ok(entries) => {
                                            s.skills = entries.iter().map(|e| {
                                                crate::state::SkillDisplayEntry {
                                                    name: e.name.clone(),
                                                    version: e.version.clone(),
                                                    scope: e.scope.clone(),
                                                    description: e.description.clone(),
                                                }
                                            }).collect();
                                            s.error = None;
                                        }
                                        Err(e) => { s.error = Some(e); }
                                    }
                                });
                            });
                        },
                        "Retry"
                    }
                }
            }
        };
    }

    if count == 0 && !loading {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No skills discovered" } } };
    }

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            div { class: "flex items-center justify-between mb-2",
                div { class: "text-[12px] text-[#888]", "Skills ({count})" }
                button {
                    class: "px-2 py-0.5 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                    onclick: move |_| {
                        let client = rpc_client.clone();
                        let sig = signal;
                        safe_write(sig, |s| { s.loading = true; s.error = None; });
                        let client2 = client.clone();
                        client2.skill_refresh(move |result| {
                            match result {
                                Ok(_) => {
                                    let client3 = client.clone();
                                    let sig3 = sig;
                                    client3.skill_list(move |list_result| {
                                        safe_write(sig3, |s| {
                                            s.loading = false;
                                            match list_result {
                                                Ok(entries) => {
                                                    s.skills = entries.iter().map(|e| {
                                                        crate::state::SkillDisplayEntry {
                                                            name: e.name.clone(),
                                                            version: e.version.clone(),
                                                            scope: e.scope.clone(),
                                                            description: e.description.clone(),
                                                        }
                                                    }).collect();
                                                    s.error = None;
                                                }
                                                Err(e) => { s.error = Some(e); }
                                            }
                                        });
                                    });
                                }
                                Err(e) => {
                                    safe_write(sig, |s| { s.loading = false; s.error = Some(e); });
                                }
                            }
                        });
                    },
                    "Refresh"
                }
            }
            if loading {
                div { class: "text-[12px] text-[#888] mb-2", "Loading..." }
            }
            div { class: "sm:hidden flex flex-col gap-2",
                {(0..count).map(|i| {
                    let s = signal;
                    let d = dialog_signal;
                    let c = rpc_client.clone();
                    rsx! { SkillCard { signal: s, dialog_signal: d, rpc_client: c, index: i } }
                }).collect::<Vec<Element>>().into_iter()}
            }
            table { class: "hidden sm:table w-full border-collapse",
                thead { tr {
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Name" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Version" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Scope" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Description" }
                } }
                tbody {
                    {(0..count).map(|i| { let s = signal; let d = dialog_signal; let c = rpc_client.clone(); rsx! { SkillRow { signal: s, dialog_signal: d, rpc_client: c, index: i } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
}

#[component]
fn SkillCard(
    signal: Signal<SkillsState>,
    mut dialog_signal: Signal<SkillDialogState>,
    rpc_client: crate::web::client::JsonRpcClient,
    index: usize,
) -> Element {
    let skill = signal.read().skills.get(index).cloned();
    let Some(skill) = skill else { return rsx! {}; };

    let color = match skill.scope.as_str() { "User" => "#40c040", "Repo" => "#4080ff", _ => "#c0c040" };

    rsx! {
        div {
            class: "cursor-pointer rounded-md border border-[#333355] bg-[#20203a] p-3 active:bg-[#2a2a44]",
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
            div { class: "flex items-start justify-between gap-3",
                div { class: "min-w-0",
                    div { class: "truncate text-[14px] font-bold text-[#e0e0e0]", "{skill.name}" }
                    div { class: "mt-0.5 text-[11px] text-[#777]", "v{skill.version}" }
                }
                span {
                    class: "flex-shrink-0 rounded border border-[#333355] px-2 py-0.5 text-[11px] font-semibold",
                    style: "color: {color};",
                    "{skill.scope}"
                }
            }
            if !skill.description.is_empty() {
                div { class: "mt-2 text-[12px] leading-[1.45] text-[#aaa]", "{skill.description}" }
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

#[cfg(test)]
mod tests {
    #[test]
    fn skills_panel_uses_mobile_cards_and_desktop_table() {
        let source = include_str!("skills.rs");
        let mobile_cards = ["sm:hidden", "flex", "flex-col", "gap-2"].join(" ");
        let desktop_table = ["hidden", "sm:table", "w-full"].join(" ");

        assert!(source.contains(&mobile_cards));
        assert!(source.contains(&desktop_table));
    }
}
