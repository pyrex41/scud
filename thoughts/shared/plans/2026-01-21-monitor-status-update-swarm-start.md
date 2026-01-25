# Monitor Status Update & Swarm Start Implementation Plan

## Overview

Add two features to the spawn/swarm monitor TUI:
1. **Quick Status Update** - Press 'd' to mark selected agent's task as done, 'p' for pending
2. **Start Swarm** - Press 'W' to start swarm execution from within the monitor

## Current State Analysis

### TUI Architecture
- **Event loop**: `tui/mod.rs` - Pattern-matching keyboard handler
- **App state**: `tui/app.rs` - Manages session, agents, waves, storage access
- **Storage access**: App has `project_root` and `phases` cache, can update via `Storage::new().load_group().get_task_mut().set_status()`

### Status Update Pattern (from spawn_selected_tasks)
```rust
let storage = Storage::new(self.project_root.clone());
if let Ok(mut phase) = storage.load_group(tag) {
    if let Some(task) = phase.get_task_mut(task_id) {
        task.set_status(TaskStatus::Done);
        let _ = storage.update_group(tag, &phase);
    }
}
```

### Swarm Execution
- `swarm::run()` is **blocking** - takes over until all tasks complete
- Cannot run concurrently with TUI in same process
- **Solution**: Exit TUI, spawn swarm in tmux window, user can re-monitor

## What We're NOT Doing

- Running swarm concurrently with TUI (would require major architecture changes)
- Adding complex status dialogs (keeping it keyboard-driven)
- Batch status updates (one task at a time)

## Implementation Approach

### Feature 1: Quick Status Update
- Add 'd' key in Agents panel to mark selected task as Done
- Add 'p' key in Agents panel to mark selected task as Pending
- Add 'b' key in Agents panel to mark selected task as Blocked
- Update storage, refresh display, show confirmation

### Feature 2: Start Swarm
- Add 'W' key (Shift+W) to start swarm
- Exit TUI cleanly
- Spawn `scud swarm` in a new tmux window
- Print instructions for re-monitoring

---

## Phase 1: Quick Status Update

### Overview
Add keyboard shortcuts to update task status directly from the Agents panel.

### Changes Required:

#### 1.1 Add Status Update Method to App

**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Add method to update selected agent's task status

```rust
/// Update the status of the currently selected agent's task
pub fn set_selected_task_status(&mut self, new_status: TaskStatus) -> Result<()> {
    let Some(ref session) = self.session else {
        self.error = Some("No session loaded".to_string());
        return Ok(());
    };

    let agents = session.agents.clone();
    if agents.is_empty() || self.selected >= agents.len() {
        self.error = Some("No agent selected".to_string());
        return Ok(());
    }

    let agent = &agents[self.selected];
    let task_id = &agent.task_id;
    let tag = &agent.tag;

    // Update task status in storage
    let storage = Storage::new(self.project_root.clone());
    if let Ok(mut phase) = storage.load_group(tag) {
        if let Some(task) = phase.get_task_mut(task_id) {
            let old_status = task.status.as_str().to_string();
            task.set_status(new_status.clone());
            if let Err(e) = storage.update_group(tag, &phase) {
                self.error = Some(format!("Failed to save: {}", e));
                return Ok(());
            }
            // Show confirmation
            self.error = Some(format!(
                "✓ {} → {}",
                task_id,
                new_status.as_str()
            ));
        } else {
            self.error = Some(format!("Task {} not found", task_id));
        }
    } else {
        self.error = Some(format!("Failed to load phase {}", tag));
    }

    // Refresh to show updated status
    self.refresh()?;
    self.refresh_waves();

    Ok(())
}
```

#### 1.2 Add Keyboard Handlers

**File**: `scud-cli/src/commands/spawn/tui/mod.rs`
**Changes**: Add key handlers in the Agents panel section (around line 194)

```rust
// d: Mark task as Done (in Agents panel)
(_, KeyCode::Char('d')) => {
    if app.focused_panel == FocusedPanel::Agents {
        let _ = app.set_selected_task_status(TaskStatus::Done);
    }
}

// p: Mark task as Pending (in Agents panel)
(_, KeyCode::Char('p')) => {
    if app.focused_panel == FocusedPanel::Agents {
        let _ = app.set_selected_task_status(TaskStatus::Pending);
    }
}

// b: Mark task as Blocked (in Agents panel)
(_, KeyCode::Char('b')) => {
    if app.focused_panel == FocusedPanel::Agents {
        let _ = app.set_selected_task_status(TaskStatus::Blocked);
    }
}
```

#### 1.3 Update Help Overlay

**File**: `scud-cli/src/commands/spawn/tui/ui.rs`
**Changes**: Add status shortcuts to help text (in render_help_overlay around line 753)

Add to Agents panel section:
```rust
Line::from(vec![
    Span::styled(" d ", Style::default().fg(ACCENT)),
    Span::styled("Mark Done", Style::default().fg(TEXT_PRIMARY)),
]),
Line::from(vec![
    Span::styled(" p ", Style::default().fg(ACCENT)),
    Span::styled("Mark Pending", Style::default().fg(TEXT_PRIMARY)),
]),
Line::from(vec![
    Span::styled(" b ", Style::default().fg(ACCENT)),
    Span::styled("Mark Blocked", Style::default().fg(TEXT_PRIMARY)),
]),
```

#### 1.4 Update Footer Help

**File**: `scud-cli/src/commands/spawn/tui/ui.rs`
**Changes**: Update footer hint for Agents panel

Find the Agents panel help text and add:
```rust
FocusedPanel::Agents => " d Done · p Pending · b Blocked · ...",
```

### Success Criteria - Phase 1:

#### Automated Verification:
- [x] Build succeeds: `cargo build`
- [x] No clippy warnings: `cargo clippy`

#### Manual Verification:
- [ ] In Agents panel, pressing 'd' marks selected task as Done
- [ ] In Agents panel, pressing 'p' marks selected task as Pending
- [ ] In Agents panel, pressing 'b' marks selected task as Blocked
- [ ] Status change shows confirmation message briefly
- [ ] Waves panel updates to reflect new status
- [ ] `scud show <task_id>` confirms status changed

---

## Phase 2: Start Swarm from Monitor

### Overview
Add 'W' key to exit TUI and start swarm execution in a tmux window.

### Changes Required:

#### 2.1 Add Swarm Start Method to App

**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Add method to prepare swarm start

```rust
/// Prepare to start swarm - returns swarm command to run
pub fn prepare_swarm_start(&self) -> Option<(String, String)> {
    // Get tag from session or active tag
    let tag = self.session.as_ref().map(|s| s.tag.clone())
        .or_else(|| self.active_tag.clone())?;

    let session_name = self.session_name.clone();

    // Build swarm command
    let cmd = format!(
        "scud swarm --tag {} --session {}",
        tag,
        session_name.replace("swarm-", "").replace("scud-", "")
    );

    Some((cmd, tag))
}
```

#### 2.2 Add Swarm Start Handler

**File**: `scud-cli/src/commands/spawn/tui/mod.rs`
**Changes**: Add 'W' key handler that exits TUI and returns swarm info

First, modify `run_app` return type to include optional swarm command:

```rust
enum AppExit {
    Quit,
    StartSwarm { command: String, tag: String, session_name: String },
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<AppExit> {
    loop {
        // ... existing code ...

        // In key handler, add:
        // W: Start swarm
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

        // Update quit handler:
        (_, KeyCode::Char('q')) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            return Ok(AppExit::Quit);
        }
    }
}
```

#### 2.3 Handle Swarm Start After TUI Exit

**File**: `scud-cli/src/commands/spawn/tui/mod.rs`
**Changes**: Update `run()` to handle swarm start

```rust
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
        AppExit::StartSwarm { command, tag, session_name } => {
            // Print swarm start message
            println!();
            println!("{}", "Starting swarm...".cyan().bold());
            println!("Tag: {}", tag.green());
            println!();

            // Spawn swarm in tmux window
            let window_name = format!("swarm-{}", tag);
            let tmux_session = session_name.clone();

            // Create new window and run swarm
            let script = format!(
                "cd {} && {}",
                project_root.as_ref()
                    .and_then(|p| p.to_str())
                    .unwrap_or("."),
                command
            );

            let status = Command::new("tmux")
                .args([
                    "new-window",
                    "-t", &tmux_session,
                    "-n", &window_name,
                    "bash", "-c", &format!("{}; read -p 'Press enter to close...'", script),
                ])
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("Swarm started in tmux window: {}:{}", tmux_session, window_name);
                    println!();
                    println!("To attach: {}", format!("tmux attach -t {}", tmux_session).cyan());
                    println!("To monitor: {}", format!("scud monitor --swarm --session {}", session_name).cyan());
                }
                _ => {
                    println!("{}", "Failed to start swarm in tmux".red());
                    println!("Run manually: {}", command.yellow());
                }
            }

            Ok(())
        }
    }
}
```

#### 2.4 Update Help Overlay

**File**: `scud-cli/src/commands/spawn/tui/ui.rs`
**Changes**: Add swarm shortcut to help

```rust
Line::from(vec![
    Span::styled(" W ", Style::default().fg(ACCENT)),
    Span::styled("Start Swarm", Style::default().fg(TEXT_PRIMARY)),
]),
```

#### 2.5 Update Footer Help

**File**: `scud-cli/src/commands/spawn/tui/ui.rs`
**Changes**: Add 'W Swarm' to footer

### Success Criteria - Phase 2:

#### Automated Verification:
- [x] Build succeeds: `cargo build`
- [x] No clippy warnings: `cargo clippy`

#### Manual Verification:
- [ ] Pressing 'W' exits TUI cleanly
- [ ] Swarm starts in a new tmux window
- [ ] Instructions printed for attaching/monitoring
- [ ] Swarm actually runs and processes tasks

---

## Testing Strategy

### Unit Tests:
- N/A - these are UI interactions

### Integration Tests:
- Test status update persists to storage
- Test swarm command generation

### Manual Testing Steps:
1. Run `scud monitor` with some agents
2. Navigate to Agents panel
3. Press 'd' on a running agent - verify status changes to Done
4. Press 'p' on a done agent - verify status changes to Pending
5. Press 'W' - verify TUI exits and swarm starts
6. Verify swarm processes tasks correctly

## Performance Considerations

- Status updates are instant (direct storage write)
- Swarm start exits TUI first to avoid blocking issues

## References

- TUI keyboard handling: `scud-cli/src/commands/spawn/tui/mod.rs:86-216`
- App state management: `scud-cli/src/commands/spawn/tui/app.rs`
- Status update pattern: `scud-cli/src/commands/spawn/tui/app.rs:1192-1197`
- Swarm entry point: `scud-cli/src/commands/swarm/mod.rs:49-63`
