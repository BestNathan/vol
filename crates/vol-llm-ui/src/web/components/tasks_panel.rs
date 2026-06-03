use dioxus::prelude::*;

use crate::state::TaskState;

pub(crate) fn status_color(status: &str) -> &'static str {
    match status {
        "pending" => "#888",
        "running" => "#4080ff",
        "completed" => "#40c040",
        "failed" => "#ff4040",
        "killed" => "#ff8800",
        _ => "#888",
    }
}

#[component]
pub fn TasksPanel(assignee_filter: Option<String>) -> Element {
    let app: crate::web::components::app::AppState = use_context();
    let task_state = use_signal(|| TaskState::new());

    let rpc = app.rpc_client.clone();
    let sig = task_state;
    let initial_assignee = assignee_filter.clone();

    // Initial load
    use_hook(move || {
        let mut s = sig;
        s.with_mut(|t| { t.loading = true; t.error = None; });
        let s2 = sig;
        rpc.task_list(None, initial_assignee.as_deref(), move |result| {
            let mut s2 = s2;
            s2.with_mut(|t| {
                t.loading = false;
                match result {
                    Ok(tasks) => { t.tasks = tasks; t.error = None; }
                    Err(e) => { t.error = Some(e); }
                }
            });
        });
    });

    // Retry on WS reconnect
    let rpc_retry = app.rpc_client.clone();
    let sig_retry = task_state;
    let assignee = assignee_filter.clone();
    use_hook(move || {
        let _sub = app.event_bus.subscribe(crate::state::UiEventKind::WsConnected, move |_| {
            let mut s = sig_retry;
            s.with_mut(|t| { t.loading = true; t.error = None; });
            let s2 = sig_retry;
            let a = assignee.clone();
            rpc_retry.task_list(None, a.as_deref(), move |result| {
                let mut s2 = s2;
                s2.with_mut(|t| {
                    t.loading = false;
                    match result {
                        Ok(tasks) => { t.tasks = tasks; t.error = None; }
                        Err(e) => { t.error = Some(e); }
                    }
                });
            });
        });
    });

    let tasks = task_state.read().tasks.clone();
    let loading = task_state.read().loading;
    let error = task_state.read().error.clone();
    let selected_task_id = task_state.read().selected_task;
    let status_filter = task_state.read().status_filter.clone();

    // Filter tasks by selected status
    let filtered: Vec<_> = if let Some(ref sf) = status_filter {
        if sf == "all" { tasks.clone() }
        else { tasks.iter().filter(|t| t.status == *sf).cloned().collect() }
    } else { tasks.clone() };

    // Empty + error
    if tasks.is_empty() && error.is_some() {
        let err = error.as_deref().unwrap_or("unknown");
        let rpc_btn = app.rpc_client.clone();
        let sig_btn = task_state;
        let a = assignee_filter.clone();
        return rsx! { div { class: "flex-1 overflow-y-auto p-3",
            div { class: "flex flex-col items-center justify-center h-full gap-3 text-center",
                div { class: "text-[#ff6060] text-[14px]", "Failed to load tasks" }
                div { class: "text-[#888] text-[12px] max-w-[300px] break-words", "{err}" }
                button {
                    class: "px-4 py-1.5 bg-[#3a3a55] text-[#ccc] rounded text-[13px] hover:bg-[#4a4a65]",
                    onclick: move |_| {
                        let mut s = sig_btn;
                        s.with_mut(|t| { t.loading = true; t.error = None; });
                        let s2 = sig_btn;
                        let a2 = a.clone();
                        rpc_btn.task_list(None, a2.as_deref(), move |result| {
                            let mut s2 = s2;
                            s2.with_mut(|t| {
                                t.loading = false;
                                match result {
                                    Ok(tasks) => { t.tasks = tasks; t.error = None; }
                                    Err(e) => { t.error = Some(e); }
                                }
                            });
                        });
                    },
                    "Retry"
                }
            }
        }};
    }

    // Loading
    if loading && tasks.is_empty() {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2",
            div { class: "flex items-center justify-center h-full text-[#666] text-[14px]", "Loading tasks..." }
        }};
    }

    rsx! {
        div { class: "flex flex-col flex-1 min-h-0 overflow-hidden",
            // Status filter bar
            div { class: "flex gap-1 p-2 border-b border-[#333355] flex-shrink-0 overflow-x-auto",
                for label in &["all", "pending", "running", "completed"] {
                    {
                        let lbl = label.to_string();
                        let is_active = status_filter.as_deref() == Some(label);
                        let cls = if is_active {
                            "px-2 py-0.5 rounded text-[11px] cursor-pointer bg-[#80a0ff] text-[#1a1a2e] whitespace-nowrap"
                        } else {
                            "px-2 py-0.5 rounded text-[11px] cursor-pointer bg-[#2a2a44] text-[#888] hover:bg-[#3a3a55] whitespace-nowrap"
                        };
                        rsx! {
                            button {
                                class: "{cls}",
                                onclick: {
                                    let mut s = task_state;
                                    let lbl2 = lbl.clone();
                                    move |_| { s.with_mut(|t| t.status_filter = Some(lbl2.clone())); }
                                },
                                "{lbl}"
                            }
                        }
                    }
                }
            }

            // Task list
            div { class: "flex-1 overflow-y-auto",
                if filtered.is_empty() {
                    div { class: "flex items-center justify-center h-full text-[#666] text-[13px]",
                        "No tasks found"
                    }
                }
                for task in &filtered {
                    {
                        let is_selected = selected_task_id == Some(task.id);
                        let row_cls = if is_selected {
                            "border-b border-[#333355] p-2 cursor-pointer bg-[#1a2a44]"
                        } else {
                            "border-b border-[#333355] p-2 cursor-pointer hover:bg-[#222240]"
                        };
                        let color = status_color(&task.status);
                        let task_id = task.id;
                        let task_id2 = task.id;
                        rsx! {
                            div {
                                key: "{task.id}",
                                class: "{row_cls}",
                                onclick: {
                                    let mut s = task_state;
                                    move |_| {
                                        s.with_mut(|t| {
                                            t.selected_task = if t.selected_task == Some(task_id2) { None } else { Some(task_id2) };
                                        });
                                    }
                                },
                                div { class: "flex items-center gap-2",
                                    span { class: "text-[11px] text-[#555] font-mono whitespace-nowrap", "t{task_id}" }
                                    span {
                                        class: "text-[10px] px-1 rounded font-bold whitespace-nowrap",
                                        style: "background: {color}; color: #1a1a2e;",
                                        "{task.status}"
                                    }
                                    span { class: "text-[13px] text-[#e0e0e0] truncate", "{task.subject}" }
                                    if let Some(ref assignee) = task.assignee {
                                        span { class: "text-[11px] text-[#666] ml-auto whitespace-nowrap", "{assignee}" }
                                    }
                                }
                                // Expanded detail
                                if is_selected {
                                    div { class: "mt-2 pl-4 text-[12px] text-[#aaa] flex flex-col gap-1",
                                        if !task.description.is_empty() {
                                            div { class: "text-[#ccc] mb-1", "{task.description}" }
                                        }
                                        if !task.dependencies.is_empty() {
                                            div { class: "text-[#888]",
                                                "Dependencies: "
                                                for (i, dep) in task.dependencies.iter().enumerate() {
                                                    span { "t{dep}" }
                                                    if i < task.dependencies.len() - 1 { span { ", " } }
                                                }
                                            }
                                        }
                                        if !task.blocks.is_empty() {
                                            div { class: "text-[#888]",
                                                "Blocks: "
                                                for (i, blk) in task.blocks.iter().enumerate() {
                                                    span { "t{blk}" }
                                                    if i < task.blocks.len() - 1 { span { ", " } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
