//! Skills panel showing available skills.

use dioxus::prelude::*;

use crate::web::components::app::AppState;

/// Skills panel listing discovered skills in a table format.
#[component]
pub fn SkillsPanel() -> Element {
    let state: AppState = use_context();
    let count = state.ui_state.peek().skills.len();

    if count == 0 {
        return rsx! {
            div { class: "skills-panel",
                div { class: "skills-empty", "No skills discovered" }
            }
        };
    }

    rsx! {
        div { class: "skills-panel",
            table { class: "skills-table",
                thead {
                    tr {
                        th { "Name" }
                        th { "Version" }
                        th { "Scope" }
                        th { "Description" }
                    }
                }
                tbody {
                    {render_skill_rows(state, count).into_iter()}
                }
            }
        }
    }
}

#[component]
fn SkillRow(state: AppState, index: usize) -> Element {
    let skill = {
        let ui = state.ui_state.peek();
        match ui.skills.get(index) {
            Some(e) => e.clone(),
            None => return rsx! {},
        }
    };

    let scope_color = match skill.scope.as_str() {
        "User" => "#40c040",
        "Repo" => "#4080ff",
        _ => "#c0c040",
    };

    rsx! {
        tr {
            td { style: "color: #e0e0e0; font-weight: bold;", "{skill.name}" }
            td { style: "color: #888;", "{skill.version}" }
            td { style: "color: {scope_color};", "{skill.scope}" }
            td { style: "color: #888;", "{skill.description}" }
        }
    }
}

fn render_skill_rows(state: AppState, count: usize) -> Vec<Element> {
    (0..count).map(|index| {
        let s = state.clone();
        rsx! {
            SkillRow { index, state: s }
        }
    }).collect()
}
