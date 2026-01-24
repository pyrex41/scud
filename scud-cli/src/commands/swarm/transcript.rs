//! Transcript extraction from Claude Code conversation logs
//!
//! Claude Code stores conversation logs in ~/.claude/projects/<project>/*.jsonl
//! Each line is a message with type (user/assistant), timestamp, and content.
//! This module extracts and formats these into readable transcripts.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptMessage {
    pub timestamp: DateTime<Utc>,
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub session_id: String,
    pub role: String, // "user" or "assistant"
    pub content: MessageContent,
}

/// Content of a message (can be text or tool use)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Structured(StructuredContent),
}

/// Structured content from API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredContent {
    pub role: Option<String>,
    pub content: Option<serde_json::Value>,
    pub model: Option<String>,
    pub usage: Option<Usage>,
    #[serde(rename = "stop_reason")]
    pub stop_reason: Option<String>,
}

/// Token usage stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// A tool call extracted from the transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// A tool result extracted from the transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
    pub timestamp: DateTime<Utc>,
}

/// Full transcript of a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub session_id: String,
    pub project_path: String,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub messages: Vec<TranscriptMessage>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

impl Transcript {
    /// Create an empty transcript
    pub fn new(session_id: &str, project_path: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            project_path: project_path.to_string(),
            started_at: None,
            ended_at: None,
            messages: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    /// Format as readable text
    pub fn to_text(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();

        writeln!(s, "# Transcript: {}", self.session_id).unwrap();
        writeln!(s).unwrap();

        if let Some(start) = self.started_at {
            writeln!(s, "Started: {}", start.format("%Y-%m-%d %H:%M:%S")).unwrap();
        }
        if let Some(end) = self.ended_at {
            writeln!(s, "Ended: {}", end.format("%Y-%m-%d %H:%M:%S")).unwrap();
        }

        writeln!(
            s,
            "Tokens: {} in / {} out",
            self.total_input_tokens, self.total_output_tokens
        )
        .unwrap();
        writeln!(s, "Tool calls: {}", self.tool_calls.len()).unwrap();
        writeln!(s).unwrap();
        writeln!(s, "---").unwrap();
        writeln!(s).unwrap();

        for msg in &self.messages {
            let role_prefix = match msg.role.as_str() {
                "user" => "## User",
                "assistant" => "## Assistant",
                _ => "## Unknown",
            };

            writeln!(s, "{} ({})", role_prefix, msg.timestamp.format("%H:%M:%S")).unwrap();
            writeln!(s).unwrap();

            match &msg.content {
                MessageContent::Text(text) => {
                    writeln!(s, "{}", text).unwrap();
                }
                MessageContent::Structured(structured) => {
                    if let Some(content) = &structured.content {
                        format_content(&mut s, content);
                    }
                }
            }

            writeln!(s).unwrap();
        }

        s
    }
}

/// Format content array (text blocks and tool uses)
fn format_content(s: &mut String, content: &serde_json::Value) {
    use std::fmt::Write;

    match content {
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    if let Some(type_val) = obj.get("type") {
                        match type_val.as_str() {
                            Some("text") => {
                                if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                                    writeln!(s, "{}", text).unwrap();
                                }
                            }
                            Some("tool_use") => {
                                let name = obj
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown");
                                writeln!(s, "**Tool: {}**", name).unwrap();
                                if let Some(input) = obj.get("input") {
                                    // Compact JSON for tool input
                                    if let Ok(json) = serde_json::to_string(input) {
                                        let truncated = if json.len() > 200 {
                                            format!("{}...", &json[..200])
                                        } else {
                                            json
                                        };
                                        writeln!(s, "```json\n{}\n```", truncated).unwrap();
                                    }
                                }
                            }
                            Some("tool_result") => {
                                let tool_id = obj
                                    .get("tool_use_id")
                                    .and_then(|id| id.as_str())
                                    .unwrap_or("unknown");
                                writeln!(s, "**Tool Result** ({})", tool_id).unwrap();
                                if let Some(content) = obj.get("content") {
                                    let text = match content {
                                        serde_json::Value::String(s) => s.clone(),
                                        _ => serde_json::to_string(content).unwrap_or_default(),
                                    };
                                    let truncated = if text.len() > 500 {
                                        format!("{}...", &text[..500])
                                    } else {
                                        text
                                    };
                                    writeln!(s, "```\n{}\n```", truncated).unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        serde_json::Value::String(text) => {
            writeln!(s, "{}", text).unwrap();
        }
        _ => {}
    }
}

/// Find Claude Code project directory
pub fn find_claude_project_dir(working_dir: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let claude_dir = home.join(".claude").join("projects");

    if !claude_dir.exists() {
        return None;
    }

    // Claude Code uses a mangled path name for the project directory
    // e.g., /home/user/scud -> -home-user-scud
    let project_name = working_dir
        .to_string_lossy()
        .replace('/', "-")
        .trim_start_matches('-')
        .to_string();

    // Try exact match first
    let exact_path = claude_dir.join(&format!("-{}", project_name));
    if exact_path.exists() {
        return Some(exact_path);
    }

    // Try without leading dash
    let no_dash_path = claude_dir.join(&project_name);
    if no_dash_path.exists() {
        return Some(no_dash_path);
    }

    // Search for matching directories
    if let Ok(entries) = fs::read_dir(&claude_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Check if the directory name contains our working dir path
            let normalized_name = name.replace('-', "/");
            if normalized_name.contains(&working_dir.to_string_lossy().to_string()) {
                return Some(entry.path());
            }
        }
    }

    None
}

/// List available session files in a project directory
pub fn list_session_files(project_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !project_dir.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(project_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            files.push(path);
        }
    }

    // Sort by modification time (most recent last)
    files.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());

    Ok(files)
}

/// Parse a JSONL file into a transcript
pub fn parse_transcript(path: &Path) -> Result<Transcript> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let project_path = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut transcript = Transcript::new(&session_id, &project_path);

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) {
            // Extract timestamp
            let timestamp = entry
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);

            // Update time range
            if transcript.started_at.is_none() || Some(timestamp) < transcript.started_at {
                transcript.started_at = Some(timestamp);
            }
            if transcript.ended_at.is_none() || Some(timestamp) > transcript.ended_at {
                transcript.ended_at = Some(timestamp);
            }

            // Extract message type
            let msg_type = entry
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");

            let uuid = entry
                .get("uuid")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();

            let parent_uuid = entry
                .get("parentUuid")
                .and_then(|u| u.as_str())
                .map(String::from);

            let session_id = entry
                .get("sessionId")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            if let Some(message) = entry.get("message") {
                // Extract usage stats from assistant messages
                if msg_type == "assistant" {
                    if let Some(usage) = message.get("usage") {
                        if let Some(input) = usage.get("input_tokens").and_then(|t| t.as_u64()) {
                            transcript.total_input_tokens += input;
                        }
                        if let Some(output) = usage.get("output_tokens").and_then(|t| t.as_u64()) {
                            transcript.total_output_tokens += output;
                        }
                    }

                    // Extract tool calls
                    if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                        for item in content {
                            if item.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                let tool_call = ToolCall {
                                    id: item
                                        .get("id")
                                        .and_then(|id| id.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    name: item
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    input: item.get("input").cloned().unwrap_or_default(),
                                    timestamp,
                                };
                                transcript.tool_calls.push(tool_call);
                            }
                        }
                    }
                }

                // Extract tool results from user messages
                if msg_type == "user" {
                    if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                        for item in content {
                            if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                let tool_result = ToolResult {
                                    tool_use_id: item
                                        .get("tool_use_id")
                                        .and_then(|id| id.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    content: item
                                        .get("content")
                                        .map(|c| match c {
                                            serde_json::Value::String(s) => s.clone(),
                                            _ => serde_json::to_string(c).unwrap_or_default(),
                                        })
                                        .unwrap_or_default(),
                                    is_error: item
                                        .get("is_error")
                                        .and_then(|e| e.as_bool())
                                        .unwrap_or(false),
                                    timestamp,
                                };
                                transcript.tool_results.push(tool_result);
                            }
                        }
                    }
                }

                // Create message content
                let content = if let Some(role) = message.get("role").and_then(|r| r.as_str()) {
                    // This is an API-style message
                    MessageContent::Structured(StructuredContent {
                        role: Some(role.to_string()),
                        content: message.get("content").cloned(),
                        model: message.get("model").and_then(|m| m.as_str()).map(String::from),
                        usage: message.get("usage").and_then(|u| serde_json::from_value(u.clone()).ok()),
                        stop_reason: message
                            .get("stop_reason")
                            .and_then(|s| s.as_str())
                            .map(String::from),
                    })
                } else if let Some(text) = message.get("content").and_then(|c| c.as_str()) {
                    // Simple text content
                    MessageContent::Text(text.to_string())
                } else {
                    continue;
                };

                transcript.messages.push(TranscriptMessage {
                    timestamp,
                    uuid,
                    parent_uuid,
                    session_id,
                    role: msg_type.to_string(),
                    content,
                });
            }
        }
    }

    Ok(transcript)
}

/// Find transcript files that overlap with a time range
pub fn find_transcripts_in_range(
    project_dir: &Path,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<PathBuf>> {
    let all_files = list_session_files(project_dir)?;
    let mut matching = Vec::new();

    for file in all_files {
        // Quick check: parse first and last lines for timestamps
        if let Ok(transcript) = parse_transcript(&file) {
            if let (Some(t_start), Some(t_end)) = (transcript.started_at, transcript.ended_at) {
                // Check for overlap
                if t_start <= end && t_end >= start {
                    matching.push(file);
                }
            }
        }
    }

    Ok(matching)
}

/// Print the full transcript with all messages and tool calls
pub fn print_full_transcript(transcript: &Transcript) {
    use colored::Colorize;

    println!();
    println!("{}", "Full Transcript".blue().bold());
    println!("{}", "═".repeat(80).blue());
    println!(
        "Session: {} | Tokens: {} in / {} out",
        transcript.session_id.cyan(),
        transcript.total_input_tokens.to_string().dimmed(),
        transcript.total_output_tokens.to_string().dimmed()
    );
    println!("{}", "═".repeat(80).blue());
    println!();

    for msg in &transcript.messages {
        let role_display = match msg.role.as_str() {
            "user" => "USER".green().bold(),
            "assistant" => "ASSISTANT".blue().bold(),
            _ => msg.role.yellow().bold(),
        };

        println!(
            "[{}] {} ─────────────────────────────────",
            msg.timestamp.format("%H:%M:%S"),
            role_display
        );
        println!();

        match &msg.content {
            MessageContent::Text(text) => {
                // Wrap long lines for readability
                for line in text.lines() {
                    println!("  {}", line);
                }
            }
            MessageContent::Structured(structured) => {
                if let Some(content) = &structured.content {
                    print_structured_content(content, 2);
                }
            }
        }

        println!();
    }
}

/// Print structured content with indentation
fn print_structured_content(content: &serde_json::Value, indent: usize) {
    use colored::Colorize;
    let pad = " ".repeat(indent);

    match content {
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    if let Some(type_val) = obj.get("type") {
                        match type_val.as_str() {
                            Some("text") => {
                                if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                                    for line in text.lines() {
                                        println!("{}{}", pad, line);
                                    }
                                }
                            }
                            Some("tool_use") => {
                                let name = obj
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("unknown");
                                let id = obj
                                    .get("id")
                                    .and_then(|id| id.as_str())
                                    .unwrap_or("");
                                println!();
                                println!(
                                    "{}{}{}",
                                    pad,
                                    "▶ TOOL CALL: ".yellow().bold(),
                                    name.cyan().bold()
                                );
                                println!("{}  ID: {}", pad, id.dimmed());
                                if let Some(input) = obj.get("input") {
                                    println!("{}  Input:", pad);
                                    if let Ok(json) = serde_json::to_string_pretty(input) {
                                        // Limit output size
                                        let lines: Vec<&str> = json.lines().collect();
                                        let max_lines = 20;
                                        for (i, line) in lines.iter().take(max_lines).enumerate() {
                                            println!("{}    {}", pad, line.dimmed());
                                            if i == max_lines - 1 && lines.len() > max_lines {
                                                println!(
                                                    "{}    {} more lines...",
                                                    pad,
                                                    (lines.len() - max_lines).to_string().yellow()
                                                );
                                            }
                                        }
                                    }
                                }
                                println!();
                            }
                            Some("tool_result") => {
                                let tool_id = obj
                                    .get("tool_use_id")
                                    .and_then(|id| id.as_str())
                                    .unwrap_or("unknown");
                                let is_error = obj
                                    .get("is_error")
                                    .and_then(|e| e.as_bool())
                                    .unwrap_or(false);
                                println!();
                                let header = if is_error {
                                    "◀ TOOL ERROR".red().bold()
                                } else {
                                    "◀ TOOL RESULT".green().bold()
                                };
                                println!("{}{} ({})", pad, header, tool_id.dimmed());
                                if let Some(content) = obj.get("content") {
                                    let text = match content {
                                        serde_json::Value::String(s) => s.clone(),
                                        _ => serde_json::to_string_pretty(content)
                                            .unwrap_or_default(),
                                    };
                                    // Limit output size
                                    let lines: Vec<&str> = text.lines().collect();
                                    let max_lines = 30;
                                    for (i, line) in lines.iter().take(max_lines).enumerate() {
                                        let display = if line.len() > 120 {
                                            format!("{}...", &line[..120])
                                        } else {
                                            line.to_string()
                                        };
                                        println!("{}  {}", pad, display.dimmed());
                                        if i == max_lines - 1 && lines.len() > max_lines {
                                            println!(
                                                "{}  {} more lines...",
                                                pad,
                                                (lines.len() - max_lines).to_string().yellow()
                                            );
                                        }
                                    }
                                }
                                println!();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        serde_json::Value::String(text) => {
            for line in text.lines() {
                println!("{}{}", pad, line);
            }
        }
        _ => {}
    }
}

/// Print a transcript summary
pub fn print_transcript_summary(transcript: &Transcript) {
    use colored::Colorize;

    println!();
    println!("{}", "Transcript".blue().bold());
    println!("{}", "═".repeat(60).blue());

    println!(
        "  {} {}",
        "Session:".dimmed(),
        transcript.session_id.cyan()
    );

    if let (Some(start), Some(end)) = (transcript.started_at, transcript.ended_at) {
        let duration = end.signed_duration_since(start);
        println!(
            "  {} {}s",
            "Duration:".dimmed(),
            duration.num_seconds().to_string().cyan()
        );
    }

    println!(
        "  {} {} in / {} out",
        "Tokens:".dimmed(),
        transcript.total_input_tokens.to_string().cyan(),
        transcript.total_output_tokens.to_string().cyan()
    );

    println!(
        "  {} {}",
        "Messages:".dimmed(),
        transcript.messages.len().to_string().cyan()
    );

    println!(
        "  {} {}",
        "Tool calls:".dimmed(),
        transcript.tool_calls.len().to_string().cyan()
    );

    // Show tool usage breakdown
    if !transcript.tool_calls.is_empty() {
        let mut tool_counts: HashMap<&str, usize> = HashMap::new();
        for call in &transcript.tool_calls {
            *tool_counts.entry(&call.name).or_insert(0) += 1;
        }

        println!();
        println!("{}", "Tool Usage".yellow().bold());
        println!("{}", "─".repeat(40).yellow());

        let mut sorted: Vec<_> = tool_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        for (tool, count) in sorted.iter().take(10) {
            println!("  {:30} {}", tool.dimmed(), count.to_string().cyan());
        }
    }

    println!();
}

/// List available transcripts for a project
pub fn list_transcripts(project_root: &Path) -> Result<()> {
    use colored::Colorize;

    let project_dir = find_claude_project_dir(project_root)
        .ok_or_else(|| anyhow::anyhow!("No Claude Code project found for this directory"))?;

    let files = list_session_files(&project_dir)?;

    if files.is_empty() {
        println!("{}", "No transcripts found.".yellow());
        return Ok(());
    }

    println!();
    println!("{}", "Available Transcripts".blue().bold());
    println!("{}", "═".repeat(60).blue());

    for (i, file) in files.iter().rev().enumerate().take(20) {
        let session_id = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        // Get file size and mod time
        if let Ok(metadata) = fs::metadata(file) {
            let size_kb = metadata.len() / 1024;
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| chrono::DateTime::<Utc>::from(t).format("%Y-%m-%d %H:%M").to_string().into());

            println!(
                "  {} {} ({} KB) - {}",
                format!("[{}]", i + 1).dimmed(),
                session_id.cyan(),
                size_kb.to_string().dimmed(),
                modified.unwrap_or_else(|| "unknown".to_string()).dimmed()
            );
        } else {
            println!("  {} {}", format!("[{}]", i + 1).dimmed(), session_id.cyan());
        }
    }

    if files.len() > 20 {
        println!("  ... and {} more", (files.len() - 20).to_string().dimmed());
    }

    println!();
    println!(
        "Use {} to view a transcript",
        "scud transcript --session <id>".cyan()
    );
    println!();

    Ok(())
}

/// View a specific transcript or the latest one
pub fn view_transcript(project_root: &Path, session: Option<&str>, full: bool) -> Result<()> {
    use colored::Colorize;

    let project_dir = find_claude_project_dir(project_root)
        .ok_or_else(|| anyhow::anyhow!("No Claude Code project found for this directory"))?;

    let files = list_session_files(&project_dir)?;

    if files.is_empty() {
        anyhow::bail!("No transcripts found");
    }

    // Find the requested session or use the latest
    let transcript_path = if let Some(session_id) = session {
        files
            .iter()
            .find(|f| {
                f.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s == session_id || s.contains(session_id))
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?
            .clone()
    } else {
        // Use the latest (last in sorted list)
        files.last().cloned().ok_or_else(|| anyhow::anyhow!("No transcripts found"))?
    };

    println!(
        "{}",
        format!("Loading transcript: {}", transcript_path.display()).dimmed()
    );

    let transcript = parse_transcript(&transcript_path)?;

    if full {
        print_full_transcript(&transcript);
    } else {
        print_transcript_summary(&transcript);
    }

    Ok(())
}

/// Export transcript as JSON
pub fn export_transcript_json(project_root: &Path, session: Option<&str>) -> Result<String> {
    let project_dir = find_claude_project_dir(project_root)
        .ok_or_else(|| anyhow::anyhow!("No Claude Code project found for this directory"))?;

    let files = list_session_files(&project_dir)?;

    if files.is_empty() {
        anyhow::bail!("No transcripts found");
    }

    let transcript_path = if let Some(session_id) = session {
        files
            .iter()
            .find(|f| {
                f.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s == session_id || s.contains(session_id))
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?
            .clone()
    } else {
        files.last().cloned().ok_or_else(|| anyhow::anyhow!("No transcripts found"))?
    };

    let transcript = parse_transcript(&transcript_path)?;
    serde_json::to_string_pretty(&transcript).map_err(|e| anyhow::anyhow!("JSON error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcript_new() {
        let transcript = Transcript::new("session-1", "project-1");
        assert_eq!(transcript.session_id, "session-1");
        assert!(transcript.messages.is_empty());
    }

    #[test]
    fn test_format_content_text() {
        let content = serde_json::json!([
            {"type": "text", "text": "Hello world"}
        ]);

        let mut s = String::new();
        format_content(&mut s, &content);
        assert!(s.contains("Hello world"));
    }

    #[test]
    fn test_format_content_tool_use() {
        let content = serde_json::json!([
            {
                "type": "tool_use",
                "name": "Read",
                "input": {"file_path": "/test/file.txt"}
            }
        ]);

        let mut s = String::new();
        format_content(&mut s, &content);
        assert!(s.contains("Tool: Read"));
        assert!(s.contains("file_path"));
    }
}
