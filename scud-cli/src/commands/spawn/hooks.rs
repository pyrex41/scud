//! Claude Code hook integration for spawn sessions
//!
//! Installs and manages hooks that automatically:
//! - Mark tasks as done when Claude Code finishes (Stop hook)
//! - Track which task an agent is working on via environment variables

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// Check if SCUD hooks are installed in the project's Claude settings
pub fn hooks_installed(project_root: &Path) -> bool {
    let settings_path = project_root.join(".claude").join("settings.local.json");

    if !settings_path.exists() {
        return false;
    }

    match fs::read_to_string(&settings_path) {
        Ok(content) => {
            if let Ok(settings) = serde_json::from_str::<Value>(&content) {
                // Check for our Stop hook
                settings
                    .get("hooks")
                    .and_then(|h| h.get("Stop"))
                    .and_then(|s| s.as_array())
                    .map(|arr| {
                        arr.iter().any(|hook| {
                            hook.get("hooks")
                                .and_then(|h| h.as_array())
                                .map(|cmds| {
                                    cmds.iter().any(|cmd| {
                                        cmd.get("command")
                                            .and_then(|c| c.as_str())
                                            .map(|s| s.contains("scud") && s.contains("set-status"))
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Install SCUD hooks into the project's Claude settings
pub fn install_hooks(project_root: &Path) -> Result<()> {
    let claude_dir = project_root.join(".claude");
    let settings_path = claude_dir.join("settings.local.json");

    // Ensure .claude directory exists
    fs::create_dir_all(&claude_dir).context("Failed to create .claude directory")?;

    // Load existing settings or create new
    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Build the Stop hook that reads SCUD_TASK_ID and marks task done
    // The hook command:
    // 1. Checks if SCUD_TASK_ID is set
    // 2. If set, marks the task as done
    let stop_hook = json!([
        {
            "matcher": "",
            "hooks": [
                {
                    "type": "command",
                    // Read task ID from env and mark done
                    // Uses bash to check env var and conditionally run scud
                    "command": "bash -c 'if [ -n \"$SCUD_TASK_ID\" ]; then scud set-status \"$SCUD_TASK_ID\" done 2>/dev/null || true; fi'",
                    "timeout": 10
                }
            ]
        }
    ]);

    // Merge with existing hooks (preserve other hooks)
    let hooks = settings.get("hooks").cloned().unwrap_or_else(|| json!({}));

    let mut hooks_obj = hooks.as_object().cloned().unwrap_or_default();
    hooks_obj.insert("Stop".to_string(), stop_hook);

    settings["hooks"] = json!(hooks_obj);

    // Write back
    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, content)?;

    Ok(())
}

/// Uninstall SCUD hooks from the project's Claude settings
pub fn uninstall_hooks(project_root: &Path) -> Result<()> {
    let settings_path = project_root.join(".claude").join("settings.local.json");

    if !settings_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&settings_path)?;
    let mut settings: Value = serde_json::from_str(&content)?;

    // Remove Stop hook if it's ours
    if let Some(hooks) = settings.get_mut("hooks") {
        if let Some(hooks_obj) = hooks.as_object_mut() {
            // Check if Stop hook contains our scud command before removing
            if let Some(stop) = hooks_obj.get("Stop") {
                let is_ours = stop
                    .as_array()
                    .map(|arr| {
                        arr.iter().any(|h| {
                            h.get("hooks")
                                .and_then(|cmds| cmds.as_array())
                                .map(|cmds| {
                                    cmds.iter().any(|cmd| {
                                        cmd.get("command")
                                            .and_then(|c| c.as_str())
                                            .map(|s| s.contains("SCUD_TASK_ID"))
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);

                if is_ours {
                    hooks_obj.remove("Stop");
                }
            }
        }
    }

    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, content)?;

    Ok(())
}

/// Generate environment setup for a spawned agent
/// Returns the env var that should be set for the agent
pub fn agent_env_setup(task_id: &str) -> String {
    format!("export SCUD_TASK_ID=\"{}\"", task_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_hooks_not_installed_missing_file() {
        let tmp = TempDir::new().unwrap();
        assert!(!hooks_installed(tmp.path()));
    }

    #[test]
    fn test_install_hooks_creates_settings() {
        let tmp = TempDir::new().unwrap();

        install_hooks(tmp.path()).unwrap();

        let settings_path = tmp.path().join(".claude").join("settings.local.json");
        assert!(settings_path.exists());

        let content = fs::read_to_string(&settings_path).unwrap();
        assert!(content.contains("SCUD_TASK_ID"));
        assert!(content.contains("scud"));
    }

    #[test]
    fn test_hooks_installed_detects_our_hook() {
        let tmp = TempDir::new().unwrap();

        install_hooks(tmp.path()).unwrap();
        assert!(hooks_installed(tmp.path()));
    }

    #[test]
    fn test_uninstall_hooks() {
        let tmp = TempDir::new().unwrap();

        install_hooks(tmp.path()).unwrap();
        assert!(hooks_installed(tmp.path()));

        uninstall_hooks(tmp.path()).unwrap();
        assert!(!hooks_installed(tmp.path()));
    }

    #[test]
    fn test_agent_env_setup() {
        let env = agent_env_setup("auth:5");
        assert_eq!(env, "export SCUD_TASK_ID=\"auth:5\"");
    }
}
