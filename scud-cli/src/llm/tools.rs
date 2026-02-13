//! Tool definitions and execution for the direct API agentic loop.
//!
//! Implements the core tools (Read, Write, Edit, Bash, Search, Find) that
//! the agent can use during code generation. Tool schemas match Claude Code's
//! exact parameter names so the model already knows how to use them.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Tool definition for the Anthropic API
#[derive(Debug, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Result of executing a tool
#[derive(Debug)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

/// Return all tool definitions as structs
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_tool_def(),
        write_tool_def(),
        edit_tool_def(),
        bash_tool_def(),
        search_tool_def(),
        find_tool_def(),
    ]
}

/// Return tool definitions as JSON values for API requests
pub fn tool_definitions_json() -> Vec<Value> {
    tool_definitions()
        .into_iter()
        .map(|td| {
            json!({
                "name": td.name,
                "description": td.description,
                "input_schema": td.input_schema,
            })
        })
        .collect()
}

/// Execute a tool call and return the result
pub async fn execute_tool(name: &str, input: &Value, working_dir: &Path) -> ToolResult {
    match name {
        "Read" => execute_read(input, working_dir).await,
        "Write" => execute_write(input, working_dir).await,
        "Edit" => execute_edit(input, working_dir).await,
        "Bash" => execute_bash(input, working_dir).await,
        "Search" => execute_search(input, working_dir).await,
        "Find" => execute_find(input, working_dir).await,
        _ => ToolResult {
            content: format!("Unknown tool: {}", name),
            is_error: true,
        },
    }
}

/// Create a compact summary of tool input for display
pub fn summarize_input(input: &Value) -> String {
    match input {
        Value::Object(map) => {
            if let Some(path) = map.get("file_path").and_then(|v| v.as_str()) {
                return path.to_string();
            }
            if let Some(cmd) = map.get("command").and_then(|v| v.as_str()) {
                let truncated = if cmd.len() > 80 {
                    format!("{}...", &cmd[..77])
                } else {
                    cmd.to_string()
                };
                return truncated;
            }
            if let Some(pattern) = map.get("pattern").and_then(|v| v.as_str()) {
                return format!("pattern: {}", pattern);
            }
            format!("{}", input)
        }
        _ => format!("{}", input),
    }
}

/// Resolve a file path - absolute paths pass through, relative resolved against working_dir
pub fn resolve_path(file_path: &str, working_dir: &Path) -> PathBuf {
    let path = Path::new(file_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(file_path)
    }
}

// --- Tool Definitions ---

fn read_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "Read".to_string(),
        description: "Read a file from the filesystem. Returns file contents with line numbers."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-indexed)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            },
            "required": ["file_path"]
        }),
    }
}

fn write_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "Write".to_string(),
        description: "Write content to a file. Creates parent directories if needed.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        }),
    }
}

fn edit_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "Edit".to_string(),
        description:
            "Perform an exact string replacement in a file. The old_string must be unique in the file."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact text to find and replace (must be unique in file)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement text"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        }),
    }
}

fn bash_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "Bash".to_string(),
        description: "Execute a bash command in the working directory. Commands time out after 120 seconds by default.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 120000)"
                }
            },
            "required": ["command"]
        }),
    }
}

fn search_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "Search".to_string(),
        description:
            "Search file contents using ripgrep (rg). Returns matching lines with file paths and line numbers."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in (default: working directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. \"*.rs\", \"*.{ts,tsx}\")"
                },
                "context": {
                    "type": "integer",
                    "description": "Number of context lines to show around matches"
                }
            },
            "required": ["pattern"]
        }),
    }
}

fn find_tool_def() -> ToolDefinition {
    ToolDefinition {
        name: "Find".to_string(),
        description: "Find files by name pattern using fd. Returns matching file paths."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Pattern to match file names against"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: working directory)"
                }
            },
            "required": ["pattern"]
        }),
    }
}

// --- Tool Implementations ---

async fn execute_read(input: &Value, working_dir: &Path) -> ToolResult {
    let file_path = input
        .get("file_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let path = resolve_path(file_path, working_dir);

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let offset = input
                .get("offset")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let limit = input.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

            let lines: Vec<&str> = content.lines().collect();
            let start = offset.min(lines.len());
            let end = limit
                .map(|l| (start + l).min(lines.len()))
                .unwrap_or(lines.len());

            let numbered: Vec<String> = lines[start..end]
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
                .collect();

            ToolResult {
                content: numbered.join("\n"),
                is_error: false,
            }
        }
        Err(e) => ToolResult {
            content: format!("Error reading {}: {}", path.display(), e),
            is_error: true,
        },
    }
}

async fn execute_write(input: &Value, working_dir: &Path) -> ToolResult {
    let file_path = input
        .get("file_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let content = input
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let path = resolve_path(file_path, working_dir);

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match std::fs::write(&path, content) {
        Ok(_) => ToolResult {
            content: format!("Wrote {} bytes to {}", content.len(), path.display()),
            is_error: false,
        },
        Err(e) => ToolResult {
            content: format!("Error writing {}: {}", path.display(), e),
            is_error: true,
        },
    }
}

async fn execute_edit(input: &Value, working_dir: &Path) -> ToolResult {
    let file_path = input
        .get("file_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let old_string = input
        .get("old_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new_string = input
        .get("new_string")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let path = resolve_path(file_path, working_dir);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                content: format!("Error reading: {}", e),
                is_error: true,
            }
        }
    };

    let count = content.matches(old_string).count();
    if count == 0 {
        return ToolResult {
            content: "old_string not found in file".into(),
            is_error: true,
        };
    }
    if count > 1 {
        return ToolResult {
            content: format!(
                "old_string found {} times (must be unique). Provide more context.",
                count
            ),
            is_error: true,
        };
    }

    let new_content = content.replacen(old_string, new_string, 1);
    match std::fs::write(&path, new_content) {
        Ok(_) => ToolResult {
            content: format!("Edited {}", path.display()),
            is_error: false,
        },
        Err(e) => ToolResult {
            content: format!("Error writing: {}", e),
            is_error: true,
        },
    }
}

async fn execute_bash(input: &Value, working_dir: &Path) -> ToolResult {
    let command = input
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let timeout_ms = input
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(120_000);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut result = String::new();
            if !stdout.is_empty() {
                result.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str("STDERR:\n");
                result.push_str(&stderr);
            }
            if result.len() > 30_000 {
                result.truncate(30_000);
                result.push_str("\n... (truncated)");
            }
            ToolResult {
                content: result,
                is_error: !output.status.success(),
            }
        }
        Ok(Err(e)) => ToolResult {
            content: format!("Failed to execute: {}", e),
            is_error: true,
        },
        Err(_) => ToolResult {
            content: format!("Command timed out after {}ms", timeout_ms),
            is_error: true,
        },
    }
}

async fn execute_search(input: &Value, working_dir: &Path) -> ToolResult {
    let pattern = input
        .get("pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let path = input.get("path").and_then(|v| v.as_str());
    let glob_filter = input.get("glob").and_then(|v| v.as_str());
    let context_lines = input.get("context").and_then(|v| v.as_u64());

    let search_path = path
        .map(|p| resolve_path(p, working_dir))
        .unwrap_or_else(|| working_dir.to_path_buf());

    let mut cmd = tokio::process::Command::new("rg");
    cmd.arg("--no-heading")
        .arg("--line-number")
        .arg("--color=never");

    if let Some(g) = glob_filter {
        cmd.arg("--glob").arg(g);
    }
    if let Some(c) = context_lines {
        cmd.arg("-C").arg(c.to_string());
    }

    cmd.arg(pattern).arg(&search_path);

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut result = stdout.to_string();
            if result.len() > 30_000 {
                result.truncate(30_000);
                result.push_str("\n... (truncated)");
            }
            ToolResult {
                content: result,
                is_error: false,
            }
        }
        Err(e) => ToolResult {
            content: format!("Search failed: {}", e),
            is_error: true,
        },
    }
}

async fn execute_find(input: &Value, working_dir: &Path) -> ToolResult {
    let pattern = input
        .get("pattern")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let path = input.get("path").and_then(|v| v.as_str());

    let search_path = path
        .map(|p| resolve_path(p, working_dir))
        .unwrap_or_else(|| working_dir.to_path_buf());

    let mut cmd = tokio::process::Command::new("fd");
    cmd.arg("--color=never").arg(pattern).arg(&search_path);

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            ToolResult {
                content: stdout.to_string(),
                is_error: false,
            }
        }
        Err(e) => ToolResult {
            content: format!("Find failed: {}", e),
            is_error: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_absolute() {
        let working_dir = Path::new("/home/user/project");
        let result = resolve_path("/etc/hosts", working_dir);
        assert_eq!(result, PathBuf::from("/etc/hosts"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let working_dir = Path::new("/home/user/project");
        let result = resolve_path("src/main.rs", working_dir);
        assert_eq!(result, PathBuf::from("/home/user/project/src/main.rs"));
    }

    #[test]
    fn test_summarize_input_file_path() {
        let input = json!({"file_path": "/src/main.rs"});
        assert_eq!(summarize_input(&input), "/src/main.rs");
    }

    #[test]
    fn test_summarize_input_command() {
        let input = json!({"command": "cargo build"});
        assert_eq!(summarize_input(&input), "cargo build");
    }

    #[test]
    fn test_summarize_input_long_command() {
        let long_cmd = "a".repeat(100);
        let input = json!({"command": long_cmd});
        let result = summarize_input(&input);
        assert!(result.len() <= 83); // 77 chars + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_summarize_input_pattern() {
        let input = json!({"pattern": "fn main"});
        assert_eq!(summarize_input(&input), "pattern: fn main");
    }

    #[tokio::test]
    async fn test_edit_rejects_non_unique() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello world hello").unwrap();

        let input = json!({
            "file_path": file.to_str().unwrap(),
            "old_string": "hello",
            "new_string": "goodbye"
        });

        let result = execute_edit(&input, dir.path()).await;
        assert!(result.is_error);
        assert!(result.content.contains("found 2 times"));
    }

    #[tokio::test]
    async fn test_edit_rejects_missing_string() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "hello world").unwrap();

        let input = json!({
            "file_path": file.to_str().unwrap(),
            "old_string": "nonexistent",
            "new_string": "replacement"
        });

        let result = execute_edit(&input, dir.path()).await;
        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn test_read_with_line_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

        let input = json!({"file_path": file.to_str().unwrap()});
        let result = execute_read(&input, dir.path()).await;

        assert!(!result.is_error);
        assert!(result.content.contains("1\tline1"));
        assert!(result.content.contains("2\tline2"));
        assert!(result.content.contains("3\tline3"));
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("test.txt");
        std::fs::write(&file, "line1\nline2\nline3\nline4\nline5\n").unwrap();

        let input = json!({
            "file_path": file.to_str().unwrap(),
            "offset": 1,
            "limit": 2
        });
        let result = execute_read(&input, dir.path()).await;

        assert!(!result.is_error);
        assert!(result.content.contains("2\tline2"));
        assert!(result.content.contains("3\tline3"));
        assert!(!result.content.contains("1\tline1"));
        assert!(!result.content.contains("4\tline4"));
    }

    #[test]
    fn test_tool_definitions_count() {
        let defs = tool_definitions();
        assert_eq!(defs.len(), 6);
    }

    #[test]
    fn test_tool_definitions_json_count() {
        let defs = tool_definitions_json();
        assert_eq!(defs.len(), 6);
        for def in &defs {
            assert!(def.get("name").is_some());
            assert!(def.get("description").is_some());
            assert!(def.get("input_schema").is_some());
        }
    }
}
