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
        // Ctrl+S closes the session dialog
        if key.code == KeyCode::Char('s') && key.modifiers == KeyModifiers::CONTROL {
            state.session_dialog_open = false;
            return InputAction::None;
        }
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
            state.exiting = true;
            if state.is_running {
                InputAction::None // Agent is running, flag exit but don't quit yet
            } else {
                InputAction::Exit
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(modifiers: KeyModifiers, code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    fn make_state() -> UiState {
        UiState::new("test-session".into(), ".")
    }

    #[test]
    fn test_ctrl_q_exits_when_not_running() {
        let mut state = make_state();
        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('q'));
        let action = handle_key(key, &mut state, "");
        assert!(state.exiting);
        assert!(matches!(action, InputAction::Exit));
    }

    #[test]
    fn test_ctrl_q_flags_exit_when_running() {
        let mut state = make_state();
        state.is_running = true;
        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('q'));
        let action = handle_key(key, &mut state, "");
        assert!(state.exiting);
        assert!(matches!(action, InputAction::None));
    }

    #[test]
    fn test_enter_sends_when_not_running() {
        let mut state = make_state();
        let key = make_key(KeyModifiers::NONE, KeyCode::Enter);
        let action = handle_key(key, &mut state, "hello");
        assert!(matches!(action, InputAction::Send(s) if s == "hello"));
    }

    #[test]
    fn test_enter_noop_when_running() {
        let mut state = make_state();
        state.is_running = true;
        let key = make_key(KeyModifiers::NONE, KeyCode::Enter);
        let action = handle_key(key, &mut state, "hello");
        assert!(matches!(action, InputAction::None));
    }

    #[test]
    fn test_enter_noop_on_empty() {
        let mut state = make_state();
        let key = make_key(KeyModifiers::NONE, KeyCode::Enter);
        let action = handle_key(key, &mut state, "   ");
        assert!(matches!(action, InputAction::None));
    }

    #[test]
    fn test_tab_toggles_active_tab() {
        let mut state = make_state();
        let initial = state.active_tab.clone();
        let key = make_key(KeyModifiers::NONE, KeyCode::Tab);
        handle_key(key, &mut state, "");
        assert_ne!(state.active_tab, initial);
    }

    #[test]
    fn test_approval_key_clears_pending() {
        let mut state = make_state();
        state.approval_state.tool_name = Some("bash".into());
        state.approval_state.reason = Some("reason".into());
        state.approval_state.arguments = Some("args".into());
        assert!(state.approval_state.has_pending());

        let key = make_key(KeyModifiers::NONE, KeyCode::Char('a'));
        handle_key(key, &mut state, "");
        assert!(!state.approval_state.has_pending());
    }

    #[test]
    fn test_session_dialog_esc_closes() {
        let mut state = make_state();
        state.session_dialog_open = true;
        let key = make_key(KeyModifiers::NONE, KeyCode::Esc);
        handle_key(key, &mut state, "");
        assert!(!state.session_dialog_open);
    }

    #[test]
    fn test_ctrl_s_toggles_session_dialog() {
        let mut state = make_state();
        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('s'));
        handle_key(key, &mut state, "");
        assert!(state.session_dialog_open);

        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('s'));
        handle_key(key, &mut state, "");
        assert!(!state.session_dialog_open);
    }

    #[test]
    fn test_ctrl_u_toggles_unsafe_mode() {
        let mut state = make_state();
        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('u'));
        handle_key(key, &mut state, "");
        assert!(state.unsafe_mode);
        assert_eq!(state.conversation.len(), 1);

        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('u'));
        handle_key(key, &mut state, "");
        assert!(!state.unsafe_mode);
        assert_eq!(state.conversation.len(), 2);
    }

    #[test]
    fn test_scroll_bounds() {
        let mut state = make_state();
        let key = make_key(KeyModifiers::NONE, KeyCode::Up);
        handle_key(key, &mut state, "");
        assert_eq!(state.conversation_scroll, 0); // Should not go below 0

        let key = make_key(KeyModifiers::NONE, KeyCode::PageDown);
        handle_key(key, &mut state, "");
        assert_eq!(state.conversation_scroll, 10);
    }

    #[test]
    fn test_ctrl_1_2_3_4_switch_tabs() {
        use crate::state::ActiveTab;
        let mut state = make_state();

        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('1'));
        handle_key(key, &mut state, "");
        assert_eq!(state.active_tab, ActiveTab::Conversation);

        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('2'));
        handle_key(key, &mut state, "");
        assert_eq!(state.active_tab, ActiveTab::Workspace);

        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('3'));
        handle_key(key, &mut state, "");
        assert_eq!(state.active_tab, ActiveTab::Logs);

        let key = make_key(KeyModifiers::CONTROL, KeyCode::Char('4'));
        handle_key(key, &mut state, "");
        assert_eq!(state.active_tab, ActiveTab::Skills);
    }
}
