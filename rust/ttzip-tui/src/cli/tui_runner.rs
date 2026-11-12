// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Interactive terminal TUI runner session and terminal lifecycle.

use crate::app::{AppMode, AppState};
use crate::event::EventHandler;
use crate::ui;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

fn run_tui_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app_state: &mut AppState,
    event_handler: &EventHandler,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut is_dirty = true;
    loop {
        if is_dirty {
            terminal.draw(|f| {
                ui::render(f, app_state);
            })?;
            is_dirty = false;
        }

        let event = event_handler.next()?;
        let sender = event_handler.sender.clone();

        if !matches!(event, crate::event::AppEvent::Tick) || app_state.needs_tick_redraw() {
            is_dirty = true;
        }

        app_state.handle_event(event, sender);
        if app_state.current_mode == AppMode::Exiting {
            return Ok(());
        }
    }
}

/// Runs interactive terminal TUI session.
pub fn run_interactive_tui(archive_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut app_state = AppState::new(archive_path)
        .map_err(|e| format!("Failed to load archive: {:?}", e))?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let event_handler = EventHandler::new(Duration::from_millis(16));
    let loop_result = run_tui_event_loop(&mut terminal, &mut app_state, &event_handler);

    event_handler.stop();

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    loop_result
}
