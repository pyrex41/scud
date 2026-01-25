use anyhow::Result;
use chrono::Utc;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Trait for extension runners that can handle tool calls and spawn subagents
pub trait ExtensionRunner {
    /// Handle a tool call from an extension
    fn on_tool_call(&self, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value>;

    /// Spawn a subagent for handling complex tasks
    fn spawn_subagent(&self, task: &str) -> Result<String>;
}

pub fn setup_eval_workspace(
    taskset: &crate::tasksets::TaskSet,
    run_id: &str,
) -> Result<EvalWorkspace> {
    // Create temp directory for the eval run
    let workspace_dir = crate::storage::eval_home()
        .join("runs")
        .join(run_id)
        .join("workspace");
    std::fs::create_dir_all(&workspace_dir)?;

    // Initialize as git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&workspace_dir)
        .output()?;

    // Create .scud directory structure
    let scud_dir = workspace_dir.join(".scud");
    std::fs::create_dir_all(scud_dir.join("tasks"))?;

    // Write taskset SCG
    std::fs::write(scud_dir.join("tasks/tasks.scg"), &taskset.scg_content)?;

    // Set active tag
    std::fs::write(scud_dir.join("active-tag"), &taskset.name)?;

    // Initial commit
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&workspace_dir)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial eval workspace"])
        .current_dir(&workspace_dir)
        .output()?;

    Ok(EvalWorkspace {
        path: workspace_dir,
        taskset_name: taskset.name.clone(),
        run_id: run_id.to_string(),
    })
}

pub struct EvalWorkspace {
    pub path: PathBuf,
    pub taskset_name: String,
    pub run_id: String,
}

/// Run evaluation with swarm mode
pub fn run_swarm(workspace: &EvalWorkspace, agent_count: usize) -> Result<()> {
    let start = Utc::now();

    // Spawn: scud swarm --tag {taskset} --round-size {agent_count} --json-events
    let mut child = Command::new("scud")
        .args([
            "swarm",
            "--tag",
            &workspace.taskset_name,
            "--round-size",
            &agent_count.to_string(),
            "--json-events",
        ])
        .current_dir(&workspace.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Stream and collect events
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("{}", line); // Pass through for now
            }
        }
    }

    let status = child.wait()?;
    let end = Utc::now();

    println!(
        "Swarm completed in {:?}, success: {}",
        end - start,
        status.success()
    );
    Ok(())
}

/// Run evaluation with ralph (sequential) mode
pub fn run_ralph(workspace: &EvalWorkspace) -> Result<()> {
    let start = Utc::now();

    let status = Command::new("scud")
        .args(["ralph", "--tag", &workspace.taskset_name])
        .current_dir(&workspace.path)
        .status()?;

    let end = Utc::now();
    println!(
        "Ralph completed in {:?}, success: {}",
        end - start,
        status.success()
    );
    Ok(())
}

/// Generate a direct prompt containing all tasks for a single Claude session
pub fn generate_direct_prompt(workspace: &EvalWorkspace) -> Result<String> {
    // Read the SCG file to get task content
    let scg_path = workspace.path.join(".scud/tasks/tasks.scg");
    let scg_content = std::fs::read_to_string(&scg_path)
        .map_err(|e| anyhow::anyhow!("Failed to read tasks: {}", e))?;

    // Generate a comprehensive prompt for a single session
    let prompt = format!(
        r#"You are working on implementing the following tasks. Complete all of them in order, respecting dependencies.

## Tasks (SCG format)
```
{}
```

## Instructions
1. Read and understand all tasks above
2. Complete tasks in dependency order (tasks with deps must wait for their dependencies)
3. For each task:
   - Implement the required changes
   - Commit with a message like "[task-id] description"
   - Move to the next task
4. After completing all tasks, run any tests to verify

Begin implementing the tasks now."#,
        scg_content
    );

    Ok(prompt)
}

/// Run evaluation with claude-direct (single agent, no SCUD orchestration)
pub fn run_claude_direct(workspace: &EvalWorkspace) -> Result<()> {
    let start = Utc::now();

    // Generate the prompt containing all tasks
    let prompt = generate_direct_prompt(workspace)?;

    // Write prompt to a temp file for claude to read
    let prompt_file = workspace.path.join(".scud/direct-prompt.md");
    std::fs::write(&prompt_file, &prompt)?;

    // Run Claude Code with the prompt
    let status = Command::new("claude")
        .args([
            "--print",
            "--dangerously-skip-permissions",
            &prompt,
        ])
        .current_dir(&workspace.path)
        .status()?;

    let end = Utc::now();
    println!(
        "Claude-direct completed in {:?}, success: {}",
        end - start,
        status.success()
    );

    Ok(())
}
