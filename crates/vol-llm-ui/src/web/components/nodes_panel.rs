//! Nodes panel — shows registered data-plane nodes and their status.

use crate::web::client::NodeListEntry;
use crate::web::components::app::AppState;
use dioxus::prelude::*;

#[component]
pub fn NodesPanel() -> Element {
    let app: AppState = use_context();
    let mut nodes = use_signal(Vec::<NodeListEntry>::new);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        let cp = app.cp_client.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            cp.node_list(move |result| {
                let _ = tx.send(result);
            });
            match rx.await {
                Ok(Ok(n)) => nodes.set(n),
                Ok(Err(e)) => error.set(Some(e)),
                Err(_) => error.set(Some("channel closed".to_string())),
            }
        });
    });

    rsx! {
        div { class: "flex flex-col h-full p-3 overflow-auto",
            h2 { class: "text-lg font-bold mb-3 text-[#e0e0e0]", "Nodes" }
            if let Some(ref err) = *error.read() {
                div { class: "text-red-400 text-sm", "Error: {err}" }
            } else if nodes.read().is_empty() {
                div { class: "text-[#888] text-sm", "No nodes connected" }
            } else {
                for node in nodes.read().iter() {
                    NodeRow { node: node.clone() }
                }
            }
        }
    }
}

#[component]
fn NodeRow(node: NodeListEntry) -> Element {
    let status_color = if node.status == "online" {
        "bg-green-500"
    } else {
        "bg-red-500"
    };
    rsx! {
        div { class: "flex items-center gap-3 p-2 border-b border-[#333355] hover:bg-[#2a2a44] rounded",
            div { class: "w-2 h-2 rounded-full {status_color} flex-shrink-0" }
            div { class: "flex-1 min-w-0",
                div { class: "text-[#e0e0e0] text-sm font-medium truncate", "{node.name}" }
                div { class: "text-[#888] text-xs", "id: {node.node_id} · v{node.version}" }
            }
            if let Some(count) = node.agent_count {
                div { class: "text-[#888] text-xs flex-shrink-0", "{count} agents" }
            }
        }
    }
}
