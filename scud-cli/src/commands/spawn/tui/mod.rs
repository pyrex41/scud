//! TUI module for spawn session monitoring
//!
//! Split-view design: Agent list on left, live terminal output on right.
//! Press Enter for fullscreen terminal view, Esc to return.
//! Press i to enter input mode and send commands to agents.

pub mod app;
pub mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use self::app::{App, ViewMode};
use self::ui::render;

/// Run the TUI monitor
pub fn run(project_root: Option<PathBuf>, session_name: &str) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(project_root, session_name)?;

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        // Draw UI
        terminal.draw(|frame| render(frame, app))?;

        // Poll for events with timeout (allows periodic refresh)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle help overlay first
                if app.show_help {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                            app.show_help = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                // Handle input mode separately
                if app.view_mode == ViewMode::Input {
                    match key.code {
                        KeyCode::Esc => app.exit_fullscreen(),
                        KeyCode::Enter => app.send_input()?,
                        KeyCode::Backspace => app.input_backspace(),
                        KeyCode::Char(c) => app.input_char(c),
                        _ => {}
                    }
                    continue;
                }

                // Normal mode key handling
                match (key.modifiers, key.code) {
                    // Quit
                    (_, KeyCode::Char('q')) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                        return Ok(());
                    }

                    // j/k always switch agents
                    (_, KeyCode::Char('k')) => app.previous_agent(),
                    (_, KeyCode::Char('j')) => app.next_agent(),

                    // Arrow keys: scroll in fullscreen, navigate agents in split
                    (_, KeyCode::Up) => {
                        if app.view_mode == ViewMode::Fullscreen {
                            app.scroll_up(1);
                        } else {
                            app.previous_agent();
                        }
                    }
                    (_, KeyCode::Down) => {
                        if app.view_mode == ViewMode::Fullscreen {
                            app.scroll_down(1);
                        } else {
                            app.next_agent();
                        }
                    }
                    (_, KeyCode::PageUp) => app.scroll_up(10),
                    (_, KeyCode::PageDown) => app.scroll_down(10),

                    // G: jump to bottom (like vim)
                    (KeyModifiers::SHIFT, KeyCode::Char('G')) | (_, KeyCode::Char('G')) => {
                        app.scroll_to_bottom();
                    }
                    // g: jump to top
                    (_, KeyCode::Char('g')) => app.scroll_up(usize::MAX),

                    // Enter: toggle fullscreen
                    (_, KeyCode::Enter) => app.toggle_fullscreen(),

                    // Esc: exit fullscreen or do nothing in split
                    (_, KeyCode::Esc) => {
                        if app.view_mode == ViewMode::Fullscreen {
                            app.exit_fullscreen();
                        }
                    }

                    // i: Enter input mode (send text to agent)
                    (_, KeyCode::Char('i')) => app.enter_input_mode(),

                    // x: Stop/interrupt agent (Ctrl+C)
                    (_, KeyCode::Char('x')) => app.restart_agent()?,

                    // Refresh
                    (_, KeyCode::Char('r')) => {
                        app.refresh()?;
                        app.refresh_live_output();
                    }

                    // Help
                    (_, KeyCode::Char('?')) => app.toggle_help(),

                    _ => {}
                }
            }
        }

        // Periodic tick (refreshes output and status)
        app.tick()?;
    }
}
