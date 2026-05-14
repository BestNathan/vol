//! MCP Panel — displays servers, tools, resources, and prompts.

use dioxus::prelude::*;

use crate::state::{McpState, McpSubtab};
use crate::web::components::app::AppState;

#[component]
pub fn McpPanel() -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();
    let signal = use_signal(|| McpState::new());

    // Load data on mount
    use_hook(move || {
        let mut sig = signal;

        rpc_client.mcp_list_servers(move |result| {
            sig.with_mut(|s| match result {
                Ok(servers) => s.servers = servers,
                Err(e) => s.error = Some(e),
            });
            sig.with_mut(|s| s.loading = false);
        });

        let rpc_client2 = rpc_client.clone();
        let mut sig2 = sig;
        rpc_client2.mcp_list_tools(None, move |result| {
            if let Ok(tools) = result {
                sig2.with_mut(|s| s.tools = tools);
            }
        });

        let rpc_client3 = rpc_client.clone();
        let mut sig3 = sig;
        rpc_client3.mcp_list_resources(None, move |result| {
            if let Ok(resources) = result {
                sig3.with_mut(|s| s.resources = resources);
            }
        });

        let rpc_client4 = rpc_client.clone();
        let mut sig4 = sig;
        rpc_client4.mcp_list_resource_templates(None, move |result| {
            if let Ok(templates) = result {
                sig4.with_mut(|s| s.resource_templates = templates);
            }
        });

        let rpc_client5 = rpc_client;
        let mut sig5 = sig;
        rpc_client5.mcp_list_prompts(None, move |result| {
            if let Ok(prompts) = result {
                sig5.with_mut(|s| s.prompts = prompts);
            }
        });
    });

    let (active, loading) = {
        let s = signal.read();
        (s.active_subtab, s.loading)
    };

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            if loading {
                div { class: "text-[#666] text-center p-4 text-[13px]", "Loading MCP data..." }
            } else {
                div {
                    // Sub-tab buttons
                    div { class: "flex gap-1 mb-2",
                        McpSubtabButton { signal, subtab: McpSubtab::Servers, label: "Servers" }
                        McpSubtabButton { signal, subtab: McpSubtab::Tools, label: "Tools" }
                        McpSubtabButton { signal, subtab: McpSubtab::Resources, label: "Resources" }
                        McpSubtabButton { signal, subtab: McpSubtab::Prompts, label: "Prompts" }
                    }
                    // Sub-tab content
                    match active {
                        McpSubtab::Servers => rsx! { ServerList { signal, app_state } },
                        McpSubtab::Tools => rsx! { ToolList { signal } },
                        McpSubtab::Resources => rsx! { ResourceList { signal } },
                        McpSubtab::Prompts => rsx! { PromptList { signal } },
                    }
                }
            }
        }
    }
}

#[component]
fn McpSubtabButton(mut signal: Signal<McpState>, subtab: McpSubtab, label: String) -> Element {
    let active = signal.read().active_subtab == subtab;
    let class = if active {
        "px-3 py-1 bg-[#1a1a2e] text-[#e0e0e0] rounded text-[12px] cursor-pointer border border-[#80a0ff]"
    } else {
        "px-3 py-1 bg-transparent text-[#888] rounded text-[12px] cursor-pointer hover:text-[#ccc] hover:bg-[#2a2a44]"
    };
    rsx! {
        button {
            class,
            onclick: move |_| { signal.write_unchecked().active_subtab = subtab; },
            "{label}"
        }
    }
}

#[component]
fn ServerList(signal: Signal<McpState>, app_state: AppState) -> Element {
    let (servers, error) = {
        let s = signal.read();
        (s.servers.clone(), s.error.clone())
    };

    if servers.is_empty() && error.is_none() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No MCP servers configured" }
        };
    }

    rsx! {
        div { class: "font-mono text-[13px]",
            {servers.into_iter().map(|s| {
                let sig = signal.clone();
                let app = app_state.clone();
                rsx! { ServerRow { signal: sig, server: s, app_state: app } }
            }).collect::<Vec<Element>>().into_iter()}
            if let Some(ref e) = error {
                div { class: "text-[#c04040] p-2 text-[12px]", "{e}" }
            }
        }
    }
}

#[component]
fn ServerRow(signal: Signal<McpState>, app_state: AppState, server: crate::state::McpServerInfo) -> Element {
    let status_color = match server.status.as_str() {
        "connected" => "#40c040",
        "connecting" => "#f0c040",
        "disconnected" => "#888",
        _ => "#c04040",
    };
    let show_reconnect = server.status != "connected" && server.status != "connecting";

    rsx! {
        div { class: "flex items-center justify-between py-1.5 border-b border-[#2a2a44]",
            div { class: "flex items-center gap-2",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: {status_color};" }
                span { class: "text-[13px] text-[#e0e0e0]", "{server.name}" }
                span { class: "text-[11px] text-[#666]", "{server.status}" }
            }
            if show_reconnect {
                button {
                    class: "px-2 py-0.5 bg-[#2a2a44] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                    onclick: move |_| {
                        let srv = server.name.clone();
                        let client = app_state.rpc_client.clone();
                        let sig = signal.clone();
                        let reconnect_client = client.clone();
                        reconnect_client.mcp_reconnect(&srv, move |result| {
                            if let Ok(true) = result {
                                let client2 = client.clone();
                                let mut sig2 = sig.clone();
                                client2.mcp_list_servers(move |r| {
                                    if let Ok(servers) = r {
                                        sig2.with_mut(|s| {
                                            s.servers = servers;
                                            s.error = None;
                                        });
                                    }
                                });
                                let client3 = client.clone();
                                let mut sig3 = sig;
                                client3.mcp_list_tools(None, move |r| {
                                    if let Ok(tools) = r {
                                        sig3.with_mut(|s| s.tools = tools);
                                    }
                                });
                            }
                        });
                    },
                    "Reconnect"
                }
            }
        }
    }
}

#[component]
fn ToolList(signal: Signal<McpState>) -> Element {
    let tools = signal.read().tools.clone();
    if tools.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No tools available" }
        };
    }

    let mut groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for t in &tools {
        groups.entry(t.server.clone()).or_default().push(t.clone());
    }

    rsx! {
        div { class: "font-mono text-[13px]",
            {groups.into_iter().map(|(server, tools)| {
                rsx! {
                    div { class: "mb-2",
                        div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({tools.len()} tools)" }
                        {tools.into_iter().map(|t| {
                            let sig = signal.clone();
                            rsx! { ToolCard { signal: sig, tool: t } }
                        }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

#[component]
fn ToolCard(mut signal: Signal<McpState>, tool: crate::state::McpToolInfo) -> Element {
    rsx! {
        div { class: "bg-[#252540] rounded p-2 mb-1",
            div { class: "flex items-center justify-between",
                div {
                    div { class: "text-[13px] text-[#e0e0e0]", "{tool.name}" }
                    if let Some(ref desc) = tool.description {
                        div { class: "text-[11px] text-[#888]", "{desc}" }
                    }
                }
                button {
                    class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                    onclick: move |_| {
                        let t = tool.clone();
                        signal.write_unchecked().tool_call_dialog = Some(crate::state::McpToolCallState {
                            server: t.server.clone(),
                            tool_name: t.name.clone(),
                            arguments_json: t.input_schema.as_ref().map(|v| serde_json::to_string_pretty(v).unwrap_or_default()).unwrap_or_else(|| "{}".to_string()),
                            result: None,
                            error: None,
                            loading: false,
                        });
                    },
                    "Call"
                }
            }
        }
    }
}

#[component]
fn ResourceList(signal: Signal<McpState>) -> Element {
    let resources = signal.read().resources.clone();
    let templates = {
        let s = signal.read();
        s.resource_templates.clone()
    };
    if resources.is_empty() && templates.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No resources available" }
        };
    }

    let mut resource_groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for r in &resources {
        resource_groups.entry(r.server.clone()).or_default().push(r.clone());
    }

    let mut template_groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for t in &templates {
        template_groups.entry(t.server.clone()).or_default().push(t.clone());
    }

    let all_servers: std::collections::HashSet<String> = resource_groups.keys()
        .chain(template_groups.keys())
        .cloned()
        .collect();

    rsx! {
        div { class: "font-mono text-[13px]",
            {all_servers.into_iter().map(|server| {
                let sig = signal.clone();
                let res = resource_groups.remove(&server).unwrap_or_default();
                let tmp = template_groups.remove(&server).unwrap_or_default();
                let total = res.len() + tmp.len();
                rsx! {
                    div { class: "mb-2",
                        div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({total} items)" }
                        {res.into_iter().map(|r| {
                            let sig = sig.clone();
                            rsx! { ResourceRow { signal: sig, resource: r } }
                        }).collect::<Vec<Element>>().into_iter()}
                        {tmp.into_iter().map(|t| {
                            rsx! { TemplateRow { template: t } }
                        }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

#[component]
fn ResourceRow(mut signal: Signal<McpState>, resource: crate::state::McpResourceInfo) -> Element {
    rsx! {
        div { class: "flex items-center justify-between py-1 border-b border-[#2a2a44]",
            div { class: "flex-1 min-w-0",
                div { class: "text-[13px] text-[#e0e0e0] truncate", "{resource.name}" }
                div { class: "text-[11px] text-[#666] truncate", "{resource.uri}" }
            }
            button {
                class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0] ml-2 flex-shrink-0",
                onclick: move |_| {
                    let r = resource.clone();
                    signal.write_unchecked().resource_viewer = Some(crate::state::McpResourceViewerState {
                        uri: r.uri.clone(),
                        content: None,
                        error: None,
                        loading: false,
                    });
                },
                "Read"
            }
        }
    }
}

#[component]
fn TemplateRow(template: crate::state::McpResourceTemplateInfo) -> Element {
    rsx! {
        div { class: "flex items-center py-1 border-b border-[#2a2a44] text-[#888]",
            div { class: "flex-1 min-w-0",
                div { class: "text-[13px]", "{template.name}" }
                div { class: "text-[11px] text-[#666] truncate", "{template.uri_template}" }
            }
            span { class: "text-[10px] bg-[#2a2a44] px-1 rounded ml-2", "template" }
        }
    }
}

#[component]
fn PromptList(signal: Signal<McpState>) -> Element {
    let prompts = signal.read().prompts.clone();
    if prompts.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No prompts available" }
        };
    }

    let mut groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for p in &prompts {
        groups.entry(p.server.clone()).or_default().push(p.clone());
    }

    rsx! {
        div { class: "font-mono text-[13px]",
            {groups.into_iter().map(|(server, prompts)| {
                rsx! {
                    div { class: "mb-2",
                        div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({prompts.len()} prompts)" }
                        {prompts.into_iter().map(|p| {
                            let sig = signal.clone();
                            rsx! { PromptRow { signal: sig, prompt: p } }
                        }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

#[component]
fn PromptRow(mut signal: Signal<McpState>, prompt: crate::state::McpPromptInfo) -> Element {
    rsx! {
        div { class: "flex items-center justify-between py-1 border-b border-[#2a2a44]",
            div {
                div { class: "text-[13px] text-[#e0e0e0]", "{prompt.name}" }
                if let Some(ref desc) = prompt.description {
                    div { class: "text-[11px] text-[#888]", "{desc}" }
                }
            }
            button {
                class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                onclick: move |_| {
                    let p = prompt.clone();
                    signal.write_unchecked().prompt_viewer = Some(crate::state::McpPromptViewerState {
                        server: p.server.clone(),
                        prompt_name: p.name.clone(),
                        args_json: "{}".to_string(),
                        result: None,
                        error: None,
                        loading: false,
                    });
                },
                "Get"
            }
        }
    }
}
