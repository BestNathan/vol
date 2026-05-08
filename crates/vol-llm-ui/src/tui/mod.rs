//! TUI frontend using ratatui.

pub mod input;
pub mod render;

pub use input::{handle_key, InputAction};
pub use render::render_ui;
