//! Waves panel view for TUI monitor
//!
//! Displays tasks organized by execution wave, with selection support for spawning.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph},
    Frame,
};

use super::app::{App, FocusedPanel, WaveTaskState};
use super::theme::*;

/// Render the waves panel showing tasks by execution wave
pub fn render_waves_panel(frame: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.focused_panel == FocusedPanel::Waves;
    let border_color = if is_focused {
        BORDER_ACTIVE
    } else {
        BORDER_DEFAULT
    };
    let title_color = if is_focused { ACCENT } else { TEXT_MUTED };

    let ready_count = app.ready_task_count();
    let selected_count = app.selected_task_count();
    let title = if app.swarm_mode {
        // Swarm mode: show SWARM indicator and wave count
        let wave_count = app.waves.len();
        format!(" SWARM Waves ({} waves) ", wave_count)
    } else if selected_count > 0 {
        format!(
            " Waves & Tasks ({} selected / {} ready) ",
            selected_count, ready_count
        )
    } else {
        format!(" Waves & Tasks ({} ready) ", ready_count)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Line::from(title).fg(title_color))
        .style(Style::default().bg(BG_SECONDARY))
        .padding(Padding::new(1, 1, 0, 0));

    if app.waves.is_empty() {
        let empty_msg = Paragraph::new("No actionable tasks")
            .style(Style::default().fg(TEXT_MUTED))
            .block(block);
        frame.render_widget(empty_msg, area);
        return;
    }

    // Build list items from waves
    let mut all_items: Vec<ListItem> = Vec::new();
    let mut task_index = 0;

    for wave in &app.waves {
        // Wave header - different format for swarm vs spawn mode
        let wave_header = if app.swarm_mode {
            build_swarm_wave_header(app, wave)
        } else {
            build_spawn_wave_header(wave)
        };
        all_items.push(ListItem::new(wave_header));

        // Tasks in wave
        for task in &wave.tasks {
            let is_selected_in_list = task_index == app.wave_task_index && is_focused;
            let is_selected_for_spawn = app.selected_tasks.contains(&task.id);

            let line = build_task_line(task, is_selected_in_list, is_selected_for_spawn);
            all_items.push(ListItem::new(line));
            task_index += 1;
        }
    }

    // Apply scroll offset - skip first N items
    let visible_items: Vec<ListItem> = all_items.into_iter().skip(app.wave_scroll_offset).collect();

    let list = List::new(visible_items).block(block);
    frame.render_widget(list, area);
}

/// Build the wave header line for swarm mode
fn build_swarm_wave_header(app: &App, wave: &super::app::Wave) -> Line<'static> {
    // In swarm mode, show validation status from actual wave data
    let validation_info = if let Some(ref swarm) = app.swarm_session_data {
        if let Some(wave_state) = swarm.waves.iter().find(|w| w.wave_number == wave.number) {
            match &wave_state.validation {
                Some(v) if v.all_passed => ("✓", STATUS_COMPLETED, "VALIDATED"),
                Some(_) => ("✗", FAILED_VALIDATION_RED, "FAILED"),
                None if wave_state.completed_at.is_some() => ("◌", TEXT_MUTED, "NO VALIDATION"),
                None => ("●", STATUS_RUNNING, "IN PROGRESS"),
            }
        } else {
            ("◌", TEXT_MUTED, "PENDING")
        }
    } else {
        ("◌", TEXT_MUTED, "PENDING")
    };

    let round_count = if let Some(ref swarm) = app.swarm_session_data {
        swarm
            .waves
            .iter()
            .find(|w| w.wave_number == wave.number)
            .map(|w| w.rounds.len())
            .unwrap_or(0)
    } else {
        0
    };

    let repair_count = if let Some(ref swarm) = app.swarm_session_data {
        swarm
            .waves
            .iter()
            .find(|w| w.wave_number == wave.number)
            .map(|w| w.repairs.len())
            .unwrap_or(0)
    } else {
        0
    };

    let repair_info = if repair_count > 0 {
        format!(" [{} repairs]", repair_count)
    } else {
        String::new()
    };

    Line::from(vec![
        Span::styled(
            format!("Wave {} ", wave.number),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} ", validation_info.0),
            Style::default().fg(validation_info.1),
        ),
        Span::styled(
            format!("{} ", validation_info.2),
            Style::default().fg(validation_info.1),
        ),
        Span::styled(
            format!(
                "({} tasks, {} rounds{})",
                wave.tasks.len(),
                round_count,
                repair_info
            ),
            Style::default().fg(TEXT_MUTED),
        ),
    ])
}

/// Build the wave header line for spawn mode
fn build_spawn_wave_header(wave: &super::app::Wave) -> Line<'static> {
    let ready_in_wave = wave
        .tasks
        .iter()
        .filter(|t| t.state == WaveTaskState::Ready)
        .count();

    Line::from(vec![
        Span::styled(
            format!("Wave {} ", wave.number),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({} tasks, {} ready)", wave.tasks.len(), ready_in_wave),
            Style::default().fg(TEXT_MUTED),
        ),
    ])
}

/// Build a task line for display in the waves panel
fn build_task_line(
    task: &super::app::WaveTask,
    is_selected_in_list: bool,
    is_selected_for_spawn: bool,
) -> Line<'static> {
    let state_icon = match task.state {
        WaveTaskState::Ready => ("○", STATUS_COMPLETED), // Blue circle = ready
        WaveTaskState::Running => ("●", STATUS_RUNNING), // Green filled = running
        WaveTaskState::Done => ("✓", STATUS_COMPLETED),  // Blue check = done
        WaveTaskState::Blocked => ("◌", TEXT_MUTED),     // Hollow = blocked
        WaveTaskState::InProgress => ("◐", STATUS_RUNNING), // Half = in progress
    };

    let checkbox = if is_selected_for_spawn {
        "[x]"
    } else if task.state == WaveTaskState::Ready {
        "[ ]"
    } else {
        "   "
    };

    // Truncate title
    let max_len = 40;
    let title_display = if task.title.len() > max_len {
        format!("{}…", &task.title[..max_len - 1])
    } else {
        task.title.clone()
    };

    let complexity = if task.complexity > 0 {
        format!(" [{}]", task.complexity)
    } else {
        String::new()
    };

    Line::from(vec![
        Span::styled(
            if is_selected_in_list { "▸ " } else { "  " },
            Style::default().fg(ACCENT),
        ),
        Span::styled(
            format!("{} ", checkbox),
            Style::default().fg(if is_selected_for_spawn {
                ACCENT
            } else {
                TEXT_MUTED
            }),
        ),
        Span::styled(
            format!("{} ", state_icon.0),
            Style::default().fg(state_icon.1),
        ),
        Span::styled(format!("{} ", task.id), Style::default().fg(TEXT_MUTED)),
        Span::styled(
            title_display,
            Style::default()
                .fg(if is_selected_in_list {
                    ACCENT
                } else {
                    TEXT_PRIMARY
                })
                .add_modifier(if is_selected_in_list {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
        Span::styled(complexity, Style::default().fg(TEXT_MUTED)),
    ])
}
