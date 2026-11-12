// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Main keyboard input event router and core explorer mode handlers.

pub mod modals;
pub mod recovery;

use super::modal_state::RecoveryModalState;
use super::state::AppState;
use super::types::AppMode;
use crate::event::AppEvent;
use crossbeam_channel::Sender;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ttzip_engine::runtime::cancellation::CancellationReason;

impl AppState {
    /// Handles keyboard input based on current mode state machine.
    pub fn handle_key_event(&mut self, key: KeyEvent, event_sender: Sender<AppEvent>) {
        match self.current_mode {
            AppMode::Explorer => self.handle_explorer_key(key, event_sender),
            AppMode::Search => self.handle_search_key(key),
            AppMode::Preview => self.handle_preview_key(key),
            AppMode::Progress => self.handle_progress_key(key),
            AppMode::Help => self.handle_help_key(key),
            AppMode::RepairWizard => self.handle_repair_input(key, event_sender),
            AppMode::PasswordRecovery => self.handle_recovery_input(key, event_sender),
            AppMode::ParetoBenchmark => self.handle_pareto_input(key, event_sender),
            AppMode::SplitManager => self.handle_split_input(key, event_sender),
            AppMode::Exiting => {}
        }
    }

    fn set_explorer_selection(&mut self, index: usize) {
        self.selected_index = index;
        if self.preview_content.is_some() {
            self.update_preview_content();
        }
    }

    fn handle_explorer_navigation(&mut self, code: KeyCode, visible_count: usize) -> bool {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if visible_count > 0 {
                    self.set_explorer_selection((self.selected_index + 1).min(visible_count - 1));
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if visible_count > 0 {
                    self.set_explorer_selection(self.selected_index.saturating_sub(1));
                }
                true
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.set_explorer_selection(0);
                true
            }
            KeyCode::Char('G') | KeyCode::End => {
                if visible_count > 0 {
                    self.set_explorer_selection(visible_count - 1);
                }
                true
            }
            _ => false,
        }
    }

    fn handle_explorer_modal_open(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('/') => {
                self.search_query.clear();
                self.search_results.clear();
                self.search_selected_index = 0;
                self.current_mode = AppMode::Search;
                true
            }
            KeyCode::Char('R') => {
                self.open_repair_modal();
                true
            }
            KeyCode::Char('r') => {
                if self.recovery_modal_state.is_none() {
                    self.recovery_modal_state = Some(RecoveryModalState::default());
                }
                self.current_mode = AppMode::PasswordRecovery;
                true
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                if self.pareto_modal_state.is_none() {
                    self.pareto_modal_state = Some(crate::app::ParetoModalState::new());
                }
                self.current_mode = AppMode::ParetoBenchmark;
                true
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if self.split_modal_state.is_none() {
                    let default_dir = self
                        .archive_path
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| ".".to_string());
                    self.split_modal_state = Some(crate::app::SplitModalState::new(default_dir));
                }
                self.current_mode = AppMode::SplitManager;
                true
            }
            KeyCode::Char('?') | KeyCode::Char('h') => {
                self.current_mode = AppMode::Help;
                true
            }
            _ => false,
        }
    }

    fn handle_explorer_enter(&mut self, event_sender: Sender<AppEvent>) {
        let target_node_info = self
            .vfs
            .flatten_visible()
            .get(self.selected_index)
            .map(|item| (item.node.relative_path.clone(), item.node.is_dir));
        if let Some((path, is_dir)) = target_node_info {
            if is_dir {
                self.vfs.toggle_expanded(&path);
            } else {
                self.trigger_extraction(event_sender);
            }
        }
    }

    fn handle_explorer_key(&mut self, key: KeyEvent, event_sender: Sender<AppEvent>) {
        let visible_count = self.vfs.flatten_visible().len();
        if self.handle_explorer_navigation(key.code, visible_count)
            || self.handle_explorer_modal_open(key.code)
        {
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.current_mode = AppMode::Exiting,
            KeyCode::Esc => {
                if self.preview_content.is_some() {
                    self.preview_content = None;
                } else {
                    self.current_mode = AppMode::Exiting;
                }
            }
            KeyCode::Char(' ') => {
                let target_path = self
                    .vfs
                    .flatten_visible()
                    .get(self.selected_index)
                    .map(|item| item.node.relative_path.clone());
                if let Some(path) = target_path {
                    self.vfs.toggle_selected(&path);
                }
            }
            KeyCode::Char('a') => {
                self.all_selected_toggle = !self.all_selected_toggle;
                self.vfs.select_all(self.all_selected_toggle);
                self.set_status(if self.all_selected_toggle {
                    "Selected all entries".to_string()
                } else {
                    "Deselected all entries".to_string()
                });
            }
            KeyCode::Enter => self.handle_explorer_enter(event_sender),
            KeyCode::Char('p') | KeyCode::Tab => {
                if self.preview_content.is_some() {
                    self.preview_content = None;
                    self.current_mode = AppMode::Explorer;
                } else {
                    self.update_preview_content();
                    self.current_mode = AppMode::Preview;
                }
            }
            _ => {}
        }
    }

    fn handle_preview_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('p') | KeyCode::Tab => {
                self.preview_content = None;
                self.current_mode = AppMode::Explorer;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.preview_scroll = self.preview_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.preview_scroll = self.preview_scroll.saturating_add(10);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.preview_scroll = self.preview_scroll.saturating_sub(10);
            }
            _ => {}
        }
    }

    fn handle_progress_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.cancellation_token.cancel(CancellationReason::UserRequested);
                self.set_status("Cancelling extraction safely...".to_string());
            }
            _ => {}
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('h') => {
                self.current_mode = AppMode::Explorer;
            }
            _ => {}
        }
    }
}
