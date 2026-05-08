//! Binary entry point for the Dioxus web application.
//!
//! Launches the web UI with `dioxus::launch(App)`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use vol_llm_ui::web::components::App;

fn main() {
    dioxus::launch(App);
}
