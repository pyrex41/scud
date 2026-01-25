//! TUI module for spawn session monitoring
//!
//! Three-panel design:
//! - Top: Waves/Tasks panel showing tasks by execution wave
//! - Middle: Agents panel showing running agents
//! - Bottom: Live terminal output from selected agent
//!
//! Tab switches focus between panels. Space toggles task selection for spawning.

pub mod agents;
pub mod app;
pub mod components;
pub mod header;
pub mod output;
pub mod theme;
pub mod ui;
pub mod waves;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use self::app::{App, FocusedPanel, ViewMode};
use self::ui::render;

/// Result of the TUI app exit
enum AppExit {
    /// Normal quit
    Quit,
    /// Start swarm in tmux
    StartSwarm {
        command: String,
        tag: String,
        session_name: String,
    },
}

/// Run the TUI monitor
pub fn run(project_root: Option<PathBuf>, session_name: &str, swarm_mode: bool) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(project_root.clone(), session_name, swarm_mode)?;

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

    // Handle result
    match result? {
        AppExit::Quit => Ok(()),
        AppExit::StartSwarm {
            command,
            tag,
            session_name,
        } => {
            use colored::Colorize;

            // Print swarm start message
            println!();
            println!("{}", Colorize::bold(Colorize::cyan("Starting swarm...")));
            println!("Tag: {}", Colorize::green(tag.as_str()));
            println!();

            // Spawn swarm in tmux window
            let window_name = format!("swarm-{}", tag);
            let tmux_session = session_name.clone();

            // Build script to run in tmux
            let script = format!(
                "cd {} && {}",
                project_root
                    .as_ref()
                    .and_then(|p| p.to_str())
                    .unwrap_or("."),
                command
            );

            let status = Command::new("tmux")
                .args([
                    "new-window",
                    "-t",
                    &tmux_session,
                    "-n",
                    &window_name,
                    "bash",
                    "-c",
                    &format!("{}; read -p 'Press enter to close...'", script),
                ])
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!(
                        "Swarm started in tmux window: {}:{}",
                        tmux_session, window_name
                    );
                    println!();
                    let attach_cmd = format!("tmux attach -t {}", tmux_session);
                    println!("To attach: {}", Colorize::cyan(attach_cmd.as_str()));
                    let monitor_cmd = format!("scud monitor --swarm --session {}", session_name);
                    println!("To monitor: {}", Colorize::cyan(monitor_cmd.as_str()));
                }
                _ => {
                    println!("{}", Colorize::red("Failed to start swarm in tmux"));
                    println!("Run manually: {}", Colorize::yellow(command.as_str()));
                }
            }

            Ok(())
        }
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<AppExit> {
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
                        return Ok(AppExit::Quit);
                    }

                    // Tab: switch panel focus
                    (_, KeyCode::Tab) => app.next_panel(),
                    (KeyModifiers::SHIFT, KeyCode::BackTab) => app.previous_panel(),

                    // j/k: navigate within current panel
                    (_, KeyCode::Char('k')) | (_, KeyCode::Up) => {
                        if app.view_mode == ViewMode::Fullscreen {
                            app.scroll_up(1);
                        } else {
                            app.move_up();
                        }
                    }
                    (_, KeyCode::Char('j')) | (_, KeyCode::Down) => {
                        if app.view_mode == ViewMode::Fullscreen {
                            app.scroll_down(1);
                        } else {
                            app.move_down();
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

                    // Space: toggle task selection (in waves panel)
                    (_, KeyCode::Char(' ')) => {
                        if app.focused_panel == FocusedPanel::Waves {
                            app.toggle_task_selection();
                        }
                    }

                    // a: select all ready tasks
                    (_, KeyCode::Char('a')) => {
                        if app.focused_panel == FocusedPanel::Waves {
                            app.select_all_ready();
                        }
                    }

                    // c: clear selection
                    (_, KeyCode::Char('c')) => {
                        if app.focused_panel == FocusedPanel::Waves {
                            app.clear_selection();
                        }
                    }

                    // s: spawn selected tasks
                    (_, KeyCode::Char('s')) => {
                        if app.focused_panel == FocusedPanel::Waves && app.selected_task_count() > 0
                        {
                            let count = app.selected_task_count();
                            match app.spawn_selected_tasks() {
                                Ok(spawned) if spawned > 0 => {
                                    app.error = None;
                                    // Switch to agents panel to see the new agents
                                    app.focused_panel = FocusedPanel::Agents;
                                }
                                Ok(_) => {
                                    app.error = Some(format!("Failed to spawn {} tasks", count));
                                }
                                Err(e) => {
                                    app.error = Some(format!("Spawn error: {}", e));
                                }
                            }
                        }
                    }

                    // Enter: toggle fullscreen or view agent output
                    (_, KeyCode::Enter) => {
                        if app.focused_panel == FocusedPanel::Output
                            || app.view_mode == ViewMode::Fullscreen
                        {
                            app.toggle_fullscreen();
                        } else if app.focused_panel == FocusedPanel::Agents {
                            // Switch to output panel to see agent's output
                            app.focused_panel = FocusedPanel::Output;
                            app.refresh_live_output();
                        }
                    }

                    // Esc: exit fullscreen or do nothing in split
                    (_, KeyCode::Esc) => {
                        if app.view_mode == ViewMode::Fullscreen {
                            app.exit_fullscreen();
                        }
                    }

                    // i: Enter input mode (send text to agent)
                    (_, KeyCode::Char('i')) => {
                        if app.focused_panel == FocusedPanel::Agents
                            || app.focused_panel == FocusedPanel::Output
                        {
                            app.enter_input_mode();
                        }
                    }

                    // x: Stop/interrupt agent (Ctrl+C)
                    (_, KeyCode::Char('x')) => {
                        if app.focused_panel == FocusedPanel::Agents {
                            app.restart_agent()?;
                        }
                    }

                    // Refresh
                    (_, KeyCode::Char('r')) => {
                        app.refresh()?;
                        app.refresh_waves();
                        app.refresh_live_output();
                    }

                    // Help
                    (_, KeyCode::Char('?')) => app.toggle_help(),

                    // R: Toggle Ralph mode (autonomous wave execution)
                    (KeyModifiers::SHIFT, KeyCode::Char('R')) | (_, KeyCode::Char('R')) => {
                        app.toggle_ralph_mode();
                    }

                    // d: Mark task as Done (in Agents panel)
                    (_, KeyCode::Char('d')) => {
                        if app.focused_panel == FocusedPanel::Agents {
                            let _ =
                                app.set_selected_task_status(crate::models::task::TaskStatus::Done);
                        }
                    }

                    // p: Mark task as Pending (in Agents panel)
                    (_, KeyCode::Char('p')) => {
                        if app.focused_panel == FocusedPanel::Agents {
                            let _ = app
                                .set_selected_task_status(crate::models::task::TaskStatus::Pending);
                        }
                    }

                    // b: Mark task as Blocked (in Agents panel)
                    (_, KeyCode::Char('b')) => {
                        if app.focused_panel == FocusedPanel::Agents {
                            let _ = app
                                .set_selected_task_status(crate::models::task::TaskStatus::Blocked);
                        }
                    }

                    // W: Start swarm (exits TUI and spawns swarm in tmux)
                    (KeyModifiers::SHIFT, KeyCode::Char('W')) | (_, KeyCode::Char('W')) => {
                        if let Some((cmd, tag)) = app.prepare_swarm_start() {
                            return Ok(AppExit::StartSwarm {
                                command: cmd,
                                tag,
                                session_name: app.session_name.clone(),
                            });
                        } else {
                            app.error = Some("No tag available for swarm".to_string());
                        }
                    }

                    _ => {}
                }
            }
        }

        // Periodic tick (refreshes output and status)
        app.tick()?;
    }
}
