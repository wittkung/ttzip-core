// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Password recovery modal keyboard interaction and runner dispatch.

use crate::app::modal_state::RecoveryModalState;
use crate::app::state::AppState;
use crate::app::types::AppMode;
use crate::event::AppEvent;
use crossbeam_channel::Sender;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ttzip_engine::runtime::cancellation::CancellationReason;

fn handle_recovery_dict_choice_key(state: &mut RecoveryModalState, code: KeyCode) {
    match code {
        KeyCode::Left | KeyCode::Char('h') => {
            state.dict_choice = if state.dict_choice == 0 { 2 } else { state.dict_choice - 1 };
        }
        KeyCode::Right | KeyCode::Char('l') => {
            state.dict_choice = (state.dict_choice + 1) % 3;
        }
        KeyCode::Char(c @ '1'..='3') => {
            state.dict_choice = (c as usize) - ('1' as usize);
        }
        _ => {}
    }
}

fn handle_recovery_field_key(state: &mut RecoveryModalState, key: KeyEvent) {
    let max_f = if state.dict_choice == 2 { 2 } else { 1 };
    match key.code {
        KeyCode::Tab => {
            state.selected_field = (state.selected_field + 1) % max_f;
            return;
        }
        KeyCode::BackTab => {
            state.selected_field = if state.selected_field == 0 {
                max_f - 1
            } else {
                state.selected_field - 1
            };
            return;
        }
        _ => {}
    }

    if state.selected_field == 0 {
        handle_recovery_dict_choice_key(state, key.code);
    } else if state.selected_field == 1 {
        match key.code {
            KeyCode::Backspace => {
                state.custom_dict_path.pop();
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                state.custom_dict_path.push(c);
            }
            _ => {}
        }
    }
}

impl AppState {
    fn handle_recovery_escape(&mut self) {
        let is_running = self
            .recovery_modal_state
            .as_ref()
            .map(|s| s.is_running)
            .unwrap_or(false);
        if is_running {
            self.cancellation_token.cancel(CancellationReason::UserRequested);
            if let Some(ref mut s) = self.recovery_modal_state {
                s.is_running = false;
                s.status_message = Some("Recovery cancelled".to_string());
            }
        } else {
            self.current_mode = AppMode::Explorer;
        }
    }

    fn handle_recovery_submit(&mut self, event_sender: Sender<AppEvent>) {
        let found_pwd = self
            .recovery_modal_state
            .as_ref()
            .and_then(|s| s.found_password.clone());
        let is_running = self
            .recovery_modal_state
            .as_ref()
            .map(|s| s.is_running)
            .unwrap_or(false);

        if let Some(pwd) = found_pwd {
            self.recovered_password = Some(pwd.clone());
            self.set_status(format!("Archive unlocked with password: \"{}\"", pwd));
            self.current_mode = AppMode::Explorer;
            self.recovery_modal_state = None;
        } else if is_running {
            self.cancellation_token.cancel(CancellationReason::UserRequested);
            if let Some(ref mut s) = self.recovery_modal_state {
                s.is_running = false;
                s.status_message = Some("Recovery stopped by user".to_string());
            }
        } else {
            self.start_recovery_runner(event_sender);
        }
    }

    /// Handles keyboard events when in Password Recovery mode.
    pub fn handle_recovery_input(&mut self, key: KeyEvent, event_sender: Sender<AppEvent>) {
        if self.recovery_modal_state.is_none() {
            self.recovery_modal_state = Some(RecoveryModalState::default());
        }

        let custom_input_focused = self
            .recovery_modal_state
            .as_ref()
            .is_some_and(|s| s.selected_field == 1);

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.handle_recovery_escape(),
            KeyCode::Enter => self.handle_recovery_submit(event_sender),
            KeyCode::Char(' ') if !custom_input_focused => self.handle_recovery_submit(event_sender),
            _ => {
                if let Some(ref mut state) = self.recovery_modal_state {
                    handle_recovery_field_key(state, key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use ttzip_engine::types::TTZipEncryptionMethod;
    use ttzip_engine::zip::writer::{assemble_zip_archive, compress_items_parallel, ZipInputItem};

    #[test]
    fn test_custom_dict_input_focus_preserves_space_key() {
        let items = vec![ZipInputItem {
            rel_path: "secret.txt".to_string(),
            data: b"Secret".to_vec(),
            mtime_epoch_secs: 1700000000,
            mode: 0o644,
            is_directory: false,
        }];
        let compressed =
            compress_items_parallel(items, 6, TTZipEncryptionMethod::ZipCrypto, Some("123456"), 1)
                .expect("compress");
        let zip_bytes = assemble_zip_archive(&compressed).expect("assemble");
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        tmp.write_all(&zip_bytes).expect("write");

        let mut state = AppState::new(tmp.path().to_path_buf()).expect("init state");
        let (tx, _rx) = crossbeam_channel::unbounded();

        state.current_mode = AppMode::PasswordRecovery;
        state.recovery_modal_state = Some(RecoveryModalState {
            dict_choice: 2,
            selected_field: 1,
            ..RecoveryModalState::default()
        });

        let target_path = "/Users/me/My Dictionaries/passwords.txt";
        for ch in target_path.chars() {
            state.handle_recovery_input(KeyEvent::from(KeyCode::Char(ch)), tx.clone());
        }

        let modal = state.recovery_modal_state.as_ref().expect("modal state");
        assert_eq!(modal.custom_dict_path, target_path);
        assert!(!modal.is_running);
    }
}



