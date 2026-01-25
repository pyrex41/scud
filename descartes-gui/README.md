# descartes-gui

Desktop GUI for AI agent orchestration using the Ralph Wiggum loop pattern.

## Installation

```bash
cargo install descartes-gui
```

## Overview

The Descartes GUI provides visibility and control over AI agent execution, wrapping the Descartes v2 architecture with an Iced-based desktop application. It adapts the Ralph loop pattern for interactive use, giving users real-time control over agent behavior.

```
┌─────────────────────────────────────────────────────────────┐
│                      Descartes GUI                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                     │
│  │  Waves  │  │ Agents  │  │ Output  │  ← View Tabs        │
│  └─────────┘  └─────────┘  └─────────┘                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Wave 1:  [Task A] [Start]  [Task B] [Start]               │
│  Wave 2:  [Task C] [Start]  (depends on A, B)              │
│  Wave 3:  [Task D] [Start]  (depends on C)                 │
│                                                             │
│  [Refresh]                                                  │
├─────────────────────────────────────────────────────────────┤
│  Status: Running │ Current: Task A                         │
│  [Pause] [Cancel]                                          │
└─────────────────────────────────────────────────────────────┘
```

## Features

### Wave Visualization

Displays tasks organized by parallel execution waves, computed from SCUD's dependency DAG.

- **Wave grouping**: Tasks with satisfied dependencies are grouped into waves
- **Status display**: Shows task ID, title, and current status
- **One-click start**: Start button on each task to begin agent execution

### Agent Control

Real-time control over running agents:

| Control | Description |
|---------|-------------|
| **Pause** | Suspends agent execution |
| **Resume** | Continues paused agent |
| **Cancel** | Terminates agent execution |

### Live Output

Streaming display of agent output with automatic scrolling.

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | API key for Claude models |
| `XAI_API_KEY` | API key for xAI models (SCUD) |
| `RUST_LOG` | Logging level (e.g., `descartes_gui=debug`) |

### SCUD Integration

The GUI reads from SCUD storage at `.scud/tasks/tasks.scg`:

```bash
# Ensure SCUD is configured
scud warmup

# View available tasks
scud list

# Start GUI
descartes-gui
```

## Building from Source

```bash
git clone https://github.com/pyrex41/descartes
cd descartes/descartes-gui
cargo build --release
```

### System Dependencies

**macOS**: No additional dependencies required.

**Linux**:
```bash
sudo apt install libxkbcommon-dev libwayland-dev
```

## Related Crates

- [`scud-cli`](https://crates.io/crates/scud-cli) - Task management CLI
- [`scud-core`](https://crates.io/crates/scud-core) - Core library for SCUD

## License

MIT
