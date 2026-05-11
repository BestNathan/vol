//! Skills panel showing available skills.

use dioxus::prelude::*;
use crate::state::SkillsState;

#[component]
pub fn SkillsPanel() -> Element {
    let signal = use_signal(|| SkillsState::new());
    let count = signal.read().skills.len();
    if count == 0 {
        return rsx! { div { class: "skills-panel", div { class: "skills-empty", "No skills discovered" } } };
    }
    rsx! {
        div { class: "skills-panel",
            table { class: "skills-table",
                thead { tr { th { "Name" } th { "Version" } th { "Scope" } th { "Description" } } }
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
            td { style: "color: #e0e0e0; font-weight: bold;", "{skill.name}" }
            td { style: "color: #888;", "{skill.version}" }
            td { style: "color: {color};", "{skill.scope}" }
            td { style: "color: #888;", "{skill.description}" }
        }
    }
}
