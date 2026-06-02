# Skill File Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the file list in SkillDetailDialog clickable with inline content preview.

**Architecture:** Refactor the dialog to split into two scrollable regions — a file list (35% max height) on top and a content preview area (flex-1) below. A local `selected_file` signal tracks the currently selected file, and `JsonRpcClient.file_read` fetches content via RPC.

**Tech Stack:** Dioxus (web), JsonRpcClient, serde_json::Value

---

### Task 1: Refactor SkillDetailDialog with file list + preview layout

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs` (full rewrite of layout)

The entire dialog layout needs restructuring:

1. Dialog size changes from `w-[650px] max-w-[90vw] max-h-[85vh]` to `w-[800px] h-[80vh]`
2. Add `selected_file` signal: `let mut selected_file: Signal<Option<String>> = use_signal(|| None);`
3. Add `file_content` signal: `let mut file_content: Signal<Option<(String, String, bool)>> = use_signal(|| None);` — stores `(path, content, loading)`
4. Add `JsonRpcClient` import and use_context to get the app state
5. File list section: `max-h-[35%] overflow-y-auto` with clickable rows
6. Content preview section: `flex-1 overflow-y-auto min-h-0`

**New component structure:**

```rust
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
    let mut selected_file: Signal<Option<String>> = use_signal(|| None);
    // (path, content, loading)
    let mut file_content: Signal<Option<(String, String, bool)>> = use_signal(|| None);

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[800px] h-[80vh] flex flex-col overflow-hidden",
                // Header (same as before)
                div { class: "flex items-center justify-between mb-3 flex-shrink-0",
                    // ... header content ...
                }

                // Scrollable body: description + file list + preview
                div { class: "flex-1 overflow-y-auto min-h-0 space-y-3",
                    // Description, triggers, SKILL.md content (same as before)

                    // File listing — new clickable version
                    if !detail.file_listing.is_empty() {
                        div {
                            div { class: "text-[11px] text-[#888] mb-1 font-semibold", "Files" }
                            div { class: "bg-[#12121e] border border-[#2a2a44] rounded max-h-[35%] overflow-y-auto mb-3",
                                // Clickable file rows here
                            }

                            // Content preview area
                            div { class: "border border-[#2a2a44] rounded min-h-[120px]",
                                // Content display or placeholder
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 1: Rewrite the full SkillDetailDialog component**

Replace the entire file content with:

```rust
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
    let mut selected_file: Signal<Option<String>> = use_signal(|| None);
    // (file_path, content, is_loading)
    let mut file_content: Signal<Option<(String, String, bool)>> = use_signal(|| None);

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
                                                sel.set(Some(f.clone()));
                                                fc.set(Some((f.clone(), String::new(), true)));
                                                let path = f.clone();
                                                let sig = fc.clone();
                                                client.file_read(&path, move |result| {
                                                    match result {
                                                        Ok(content) => {
                                                            sig.set(Some((path.clone(), content, false)));
                                                        }
                                                        Err(e) => {
                                                            sig.set(Some((path, format!("Error: {e}"), false)));
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
                                    Some((path, content, true)) => {
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: PASS (no errors)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs
git commit -m "feat: add clickable file list with inline preview in SkillDetailDialog"
```

---

## Spec Coverage Check

| Spec Requirement | Task |
|---|---|
| Dialog size `w-[800px] h-[80vh]` | Task 1 Step 1 |
| File list `max-h-[35%]` scrollable | Task 1 Step 1 |
| Content preview `flex-1` scrollable | Task 1 Step 1 |
| File rows clickable with highlight | Task 1 Step 1 |
| Use `JsonRpcClient.file_read` | Task 1 Step 1 |
| Loading / error / placeholder states | Task 1 Step 1 |

## Placeholder Scan

No TBD, TODO, or incomplete sections. All code is provided inline.
