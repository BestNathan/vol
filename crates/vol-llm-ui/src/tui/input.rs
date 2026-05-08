// crates/vol-llm-ui/src/tui/input.rs

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::state::{UiState, ActiveTab};

/// Result of processing a key event.
pub enum InputAction {
    /// Exit the application.
    Exit,
    /// Send the input text to the agent.
    Send(String),
    /// Resume a saved session.
    ResumeSession(String),
    /// No action (key consumed for navigation).
    None,
}

/// Process a key event and return the resulting action.
pub fn handle_key(key: KeyEvent, state: &mut UiState, input_text: &str) -> InputAction {
    // Approval response keys -- highest priority
    if state.approval_state.has_pending() {
        match key.code {
            KeyCode::Char('a') | KeyCode::Char('A') => {
                state.approval_state.clear();
                return InputAction::None; // Will be handled via connection
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                state.approval_state.clear();
                return InputAction::None;
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                state.approval_state.clear();
                return InputAction::None;
            }
            _ => {}
        }
    }

    // Session dialog
    if state.session_dialog_open {
        return handle_session_dialog_key(key, state);
    }

    match (key.modifiers, key.code) {
        (KeyModifiers::ALT, KeyCode::Enter) => InputAction::None, // Insert newline

        (_, KeyCode::Enter) => {
            if state.is_running { return InputAction::None; }
            let input = input_text.trim().to_string();
            if input.is_empty() { return InputAction::None; }
            InputAction::Send(input)
        }

        (_, KeyCode::Esc) => InputAction::None, // Clear input

        (_, KeyCode::Tab) => {
            state.active_tab = state.active_tab.toggle();
            InputAction::None
        }

        (_, KeyCode::PageUp) => {
            state.conversation_scroll = state.conversation_scroll.saturating_sub(10);
            state.conversation_auto_scroll = false;
            InputAction::None
        }
        (_, KeyCode::PageDown) => {
            state.conversation_scroll = state.conversation_scroll.saturating_add(10);
            state.conversation_auto_scroll = false;
            InputAction::None
        }
        (_, KeyCode::Up) => {
            state.conversation_scroll = state.conversation_scroll.saturating_sub(1);
            state.conversation_auto_scroll = false;
            InputAction::None
        }
        (_, KeyCode::Down) => {
            state.conversation_scroll = state.conversation_scroll.saturating_add(1);
            state.conversation_auto_scroll = false;
            InputAction::None
        }

        (KeyModifiers::CONTROL, KeyCode::Char('1')) => {
            state.active_tab = ActiveTab::Conversation;
            InputAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('2')) => {
            state.active_tab = ActiveTab::Workspace;
            InputAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('3')) => {
            state.active_tab = ActiveTab::Logs;
            InputAction::None
        }
        (KeyModifiers::CONTROL, KeyCode::Char('4')) => {
            state.active_tab = ActiveTab::Skills;
            InputAction::None
        }

        (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
            if !state.is_running {
                state.session_dialog_open = !state.session_dialog_open;
            }
            InputAction::None
        }

        (_, KeyCode::Char('q')) if key.modifiers == KeyModifiers::CONTROL => {
            if !state.is_running { state.exiting = true; }
            InputAction::Exit
        }

        (_, KeyCode::Char('u')) if key.modifiers == KeyModifiers::CONTROL => {
            state.unsafe_mode = !state.unsafe_mode;
            state.conversation.push(crate::state::ConversationEntry::AgentAnswer {
                text: if state.unsafe_mode {
                    "Unsafe mode enabled -- all tool approvals auto-approved".to_string()
                } else {
                    "Unsafe mode disabled".to_string()
                },
            });
            InputAction::None
        }

        _ => InputAction::None,
    }
}

fn handle_session_dialog_key(key: KeyEvent, state: &mut UiState) -> InputAction {
    match key.code {
        KeyCode::Esc => {
            state.session_dialog_open = false;
            InputAction::None
        }
        KeyCode::Enter => {
            if let Some(entry) = state.session_dialog_sessions.get(state.session_dialog_selected) {
                let id = entry.session_id.clone();
                state.session_dialog_open = false;
                return InputAction::ResumeSession(id);
            }
            InputAction::None
        }
        KeyCode::Up => {
            if state.session_dialog_selected > 0 {
                state.session_dialog_selected -= 1;
            }
            InputAction::None
        }
        KeyCode::Down => {
            if state.session_dialog_selected + 1 < state.session_dialog_sessions.len() {
                state.session_dialog_selected += 1;
            }
            InputAction::None
        }
        KeyCode::Char('n') => {
            state.session_dialog_open = false;
            state.session_id = uuid::Uuid::new_v4().to_string();
            InputAction::None
        }
        KeyCode::Char('d') => {
            // Delete session -- placeholder, needs session store access
            if let Some(entry) = state.session_dialog_sessions.get(state.session_dialog_selected) {
                if entry.session_id != state.session_id {
                    state.session_dialog_sessions.remove(state.session_dialog_selected);
                    state.session_dialog_selected = 0.min(state.session_dialog_sessions.len().saturating_sub(1));
                }
            }
            InputAction::None
        }
        _ => InputAction::None,
    }
}
