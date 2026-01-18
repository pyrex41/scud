//! Integration Tests (US-21 to US-22)
//!
//! Tests for integration features:
//! - US-21: Claude Code Slash Commands (config agents)
//! - US-22: Interactive Browser View (view)

use crate::e2e::fixtures::TestProject;
use std::fs;

// US-21: Claude Code Slash Commands

#[test]
fn test_us21_claude_commands_directory() {
    let project = TestProject::new();

    // Create .claude/commands directory
    let commands_dir = project.path.join(".claude").join("commands").join("scud");
    fs::create_dir_all(&commands_dir).unwrap();

    // Create a sample command file
    let command_content = r#"Find the next available SCUD task and show its details.

Steps:
1. Run `scud next` to get the next task
2. If a task is available, run `scud show <id>` for full details
3. Provide a summary of what needs to be implemented
"#;

    fs::write(commands_dir.join("next.md"), command_content).unwrap();

    // Verify structure
    assert!(commands_dir.exists());
    assert!(commands_dir.join("next.md").exists());

    // Read and verify content
    let content = fs::read_to_string(commands_dir.join("next.md")).unwrap();
    assert!(content.contains("scud next"));
    assert!(content.contains("scud show"));
}

#[test]
fn test_us21_multiple_commands() {
    let project = TestProject::new();

    let commands_dir = project.path.join(".claude").join("commands").join("scud");
    fs::create_dir_all(&commands_dir).unwrap();

    // Create multiple command files
    let commands = vec![
        ("next.md", "Find next task"),
        ("show.md", "Show task details: $ARGUMENTS"),
        ("status.md", "Update task status"),
        ("list.md", "List all tasks"),
        ("stats.md", "Show statistics"),
    ];

    for (filename, content) in &commands {
        fs::write(commands_dir.join(filename), content).unwrap();
    }

    // Verify all commands created
    let entries: Vec<_> = fs::read_dir(&commands_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(entries.len(), 5);
}

#[test]
fn test_us21_command_with_arguments() {
    let project = TestProject::new();

    let commands_dir = project.path.join(".claude").join("commands").join("scud");
    fs::create_dir_all(&commands_dir).unwrap();

    let status_command = r#"Update a SCUD task status: $ARGUMENTS

Usage: /scud:status <task-id> <status>

Where status is one of: pending, in-progress, done, blocked, cancelled

Steps:
1. Parse arguments to get task ID and status
2. Run `scud set-status <task-id> <status>`
3. Confirm the status change
"#;

    fs::write(commands_dir.join("status.md"), status_command).unwrap();

    let content = fs::read_to_string(commands_dir.join("status.md")).unwrap();
    assert!(content.contains("$ARGUMENTS"));
    assert!(content.contains("set-status"));
}

#[test]
fn test_us21_claude_settings() {
    let project = TestProject::new();

    // Create .claude/settings.json
    let settings_dir = project.path.join(".claude");
    fs::create_dir_all(&settings_dir).unwrap();

    let settings = r#"{
    "allowedTools": [
        "Edit",
        "Bash(scud *)",
        "Bash(git commit:*)",
        "Bash(git add:*)"
    ]
}"#;

    fs::write(settings_dir.join("settings.json"), settings).unwrap();

    // Verify settings file
    assert!(settings_dir.join("settings.json").exists());

    let content = fs::read_to_string(settings_dir.join("settings.json")).unwrap();
    assert!(content.contains("scud"));
    assert!(content.contains("allowedTools"));
}

// US-22: Interactive Browser View

#[test]
fn test_us22_view_data_preparation() {
    let project = TestProject::wave_project();
    let phase = project.storage.load_active_group().unwrap();

    // Prepare data for view (JSON format)
    let tasks_json: Vec<_> = phase
        .tasks
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "status": format!("{:?}", t.status),
                "dependencies": t.dependencies,
                "complexity": t.complexity,
            })
        })
        .collect();

    let json_output = serde_json::to_string_pretty(&tasks_json).unwrap();

    // Verify JSON is valid and contains expected data
    assert!(json_output.contains("Research"));
    assert!(json_output.contains("Integrate"));
    assert!(json_output.contains("dependencies"));
}

#[test]
fn test_us22_mermaid_diagram_generation() {
    let project = TestProject::wave_project();
    let phase = project.storage.load_active_group().unwrap();

    // Generate Mermaid diagram
    let mut mermaid = String::from("graph TD\n");

    for task in &phase.tasks {
        // Add node
        let status_class = match task.status {
            scud::models::TaskStatus::Done => "done",
            scud::models::TaskStatus::InProgress => "inprogress",
            _ => "pending",
        };
        mermaid.push_str(&format!("    {}[\"{}\"]\n", task.id, task.title));
        mermaid.push_str(&format!("    class {} {}\n", task.id, status_class));

        // Add edges
        for dep in &task.dependencies {
            mermaid.push_str(&format!("    {} --> {}\n", dep, task.id));
        }
    }

    // Verify Mermaid output
    assert!(mermaid.contains("graph TD"));
    assert!(mermaid.contains("-->")); // Has edges
    assert!(mermaid.contains("Research"));
    assert!(mermaid.contains("Integrate"));
}

#[test]
fn test_us22_html_template() {
    let project = TestProject::simple_project();

    // Create basic HTML view template
    let html_template = r#"<!DOCTYPE html>
<html>
<head>
    <title>SCUD Task View</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
</head>
<body>
    <h1>Task Graph</h1>
    <div class="mermaid">
        {{MERMAID_DIAGRAM}}
    </div>
    <h2>Task List</h2>
    <div id="tasks">
        {{TASK_LIST}}
    </div>
</body>
</html>"#;

    // Save to project
    fs::write(project.path.join("view.html"), html_template).unwrap();

    // Verify template
    let content = fs::read_to_string(project.path.join("view.html")).unwrap();
    assert!(content.contains("mermaid"));
    assert!(content.contains("{{MERMAID_DIAGRAM}}"));
    assert!(content.contains("{{TASK_LIST}}"));
}

#[test]
fn test_us22_stats_for_view() {
    let project = TestProject::stats_project();
    let phase = project.storage.load_active_group().unwrap();
    let stats = phase.get_stats();

    // Prepare stats for view
    let stats_json = serde_json::json!({
        "total": stats.total,
        "done": stats.done,
        "pending": stats.pending,
        "in_progress": stats.in_progress,
        "blocked": stats.blocked,
        "completion_percent": (stats.done as f64 / stats.total as f64 * 100.0).round(),
    });

    let json_str = serde_json::to_string_pretty(&stats_json).unwrap();

    // Verify stats
    assert!(json_str.contains("\"total\": 6"));
    assert!(json_str.contains("\"done\": 2"));
    assert!(json_str.contains("\"pending\": 3"));
}

#[test]
fn test_us22_multi_phase_view_data() {
    let project = TestProject::multi_phase_project();
    let tasks = project.storage.load_tasks().unwrap();

    // Prepare multi-phase data for view
    let phases_data: Vec<_> = tasks
        .iter()
        .map(|(name, phase)| {
            let stats = phase.get_stats();
            serde_json::json!({
                "name": name,
                "task_count": phase.tasks.len(),
                "done": stats.done,
                "total": stats.total,
            })
        })
        .collect();

    let json_str = serde_json::to_string_pretty(&phases_data).unwrap();

    // Verify all phases included
    assert!(json_str.contains("phase1"));
    assert!(json_str.contains("phase2"));
}
