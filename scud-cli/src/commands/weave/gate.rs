use anyhow::Result;
use std::path::PathBuf;

use scud_core::weave::{Event, EventKind};

use super::check::{build_coordinator, core_storage};

/// Map a Claude Code tool name + input JSON to a weave Event.
fn map_tool_to_event(tool: &str, input: &str) -> Option<Event> {
    let input_val: serde_json::Value = serde_json::from_str(input).unwrap_or_default();

    match tool {
        "Write" | "Edit" | "NotebookEdit" => {
            let path = input_val
                .get("file_path")
                .or_else(|| input_val.get("notebook_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let kind = if is_schema_path(path) {
                EventKind::SchemaChange
            } else if is_config_path(path) {
                EventKind::ConfigChange
            } else {
                EventKind::FileWrite
            };

            Some(Event {
                kind,
                agent: std::env::var("SCUD_AGENT").ok(),
                target: Some(path.to_string()),
                task_id: std::env::var("SCUD_TASK").ok(),
                metadata: std::collections::HashMap::new(),
            })
        }
        "Bash" => {
            let command = input_val
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            map_bash_command(command)
        }
        _ => None,
    }
}

/// Map a bash command string to the appropriate event kind.
fn map_bash_command(command: &str) -> Option<Event> {
    let trimmed = command.trim();

    let (kind, target) =
        if trimmed.starts_with("git commit") || trimmed.starts_with("git merge") {
            (EventKind::Commit, "".to_string())
        } else if trimmed.starts_with("cargo add")
            || trimmed.starts_with("npm install")
            || trimmed.starts_with("yarn add")
            || trimmed.starts_with("pip install")
        {
            let dep = trimmed.split_whitespace().last().unwrap_or("");
            (EventKind::DependencyAdd, dep.to_string())
        } else if trimmed.starts_with("cargo remove")
            || trimmed.starts_with("npm uninstall")
            || trimmed.starts_with("yarn remove")
            || trimmed.starts_with("pip uninstall")
        {
            let dep = trimmed.split_whitespace().last().unwrap_or("");
            (EventKind::DependencyRemove, dep.to_string())
        } else if trimmed.starts_with("cargo build") || trimmed.starts_with("make") {
            (EventKind::Build, "".to_string())
        } else if trimmed.starts_with("cargo test")
            || trimmed.starts_with("pytest")
            || trimmed.starts_with("npm test")
            || trimmed.starts_with("jest")
        {
            (EventKind::TestRun, "".to_string())
        } else if is_dangerous_command(trimmed) {
            (EventKind::DangerousCommand, trimmed.to_string())
        } else {
            return None;
        };

    Some(Event {
        kind,
        agent: std::env::var("SCUD_AGENT").ok(),
        target: if target.is_empty() {
            None
        } else {
            Some(target)
        },
        task_id: std::env::var("SCUD_TASK").ok(),
        metadata: std::collections::HashMap::new(),
    })
}

fn is_dangerous_command(cmd: &str) -> bool {
    let dangerous = [
        "pkill",
        "kill -9",
        "kill -KILL",
        "killall",
        "rm -rf",
        "rm -fr",
        "shutdown",
        "reboot",
        "halt",
    ];
    dangerous.iter().any(|d| cmd.contains(d))
}

fn is_schema_path(path: &str) -> bool {
    let patterns = [
        "migration",
        "schema",
        "prisma/schema",
        ".sql",
        "knexfile",
    ];
    let lower = path.to_lowercase();
    patterns.iter().any(|p| lower.contains(p))
}

fn is_config_path(path: &str) -> bool {
    let patterns = [
        "Cargo.toml",
        "package.json",
        "tsconfig",
        ".env",
        "config.toml",
        "settings.json",
        "docker-compose",
        "Dockerfile",
    ];
    patterns.iter().any(|p| path.contains(p))
}

pub fn run(project_root: Option<PathBuf>, tool: &str, input: &str) -> Result<()> {
    let Some(event) = map_tool_to_event(tool, input) else {
        // Unrecognized tool/input combination -- allow by default
        std::process::exit(0);
    };

    let storage = core_storage(project_root);

    // If no weave dir or no active tag, allow by default (no weave configured)
    let weave_dir = storage.scud_dir().join("weave");
    if !weave_dir.exists() {
        std::process::exit(0);
    }

    let (coord, _tag, _phase) = match build_coordinator(&storage) {
        Ok(c) => c,
        Err(_) => {
            // Can't load coordinator -- allow rather than block
            std::process::exit(0);
        }
    };

    if coord.threads.is_empty() {
        std::process::exit(0);
    }

    let decision = coord.evaluate_full(&event);

    match decision {
        scud_core::weave::Decision::Proceed => std::process::exit(0),
        scud_core::weave::Decision::Wait { reason, thread_id } => {
            eprintln!("WAIT by {}: {}", thread_id, reason);
            std::process::exit(2);
        }
        scud_core::weave::Decision::Blocked { reason, thread_id } => {
            eprintln!("BLOCKED by {}: {}", thread_id, reason);
            std::process::exit(2);
        }
    }
}
