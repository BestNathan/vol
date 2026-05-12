//! Skills panel showing available skills.

use dioxus::prelude::*;
use crate::state::SkillsState;

#[component]
pub fn SkillsPanel() -> Element {
    let signal = use_signal(|| SkillsState::new());
    let count = signal.read().skills.len();
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
                    {(0..count).map(|i| { let s = signal.clone(); rsx! { SkillRow { signal: s, index: i } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
}

#[component]
fn SkillRow(signal: Signal<SkillsState>, index: usize) -> Element {
    let skill = signal.read().skills.get(index).cloned();
    let Some(skill) = skill else { return rsx! {}; };
    let color = match skill.scope.as_str() { "User" => "#40c040", "Repo" => "#4080ff", _ => "#c0c040" };
    rsx! {
        tr {
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#e0e0e0] font-bold", "{skill.name}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.version}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44]", style: "color: {color};", "{skill.scope}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.description}" }
        }
    }
}
