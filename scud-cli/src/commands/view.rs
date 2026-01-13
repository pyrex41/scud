use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::models::phase::Phase;
use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

// Embed the static assets at compile time
const VIEWER_STYLES: &str = include_str!("../../assets/viewer.css");
const VIEWER_SCRIPT: &str = include_str!("../../assets/viewer.js");

/// Wave data structure for the viewer
#[derive(Debug, Clone, Serialize)]
struct WaveTask {
    id: String,
    title: String,
    status: String,
    complexity: u32,
    dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Wave {
    wave: usize,
    round: usize,
    tasks: Vec<WaveTask>,
}

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD is not initialized. Run: scud init");
    }

    // Load tasks from storage
    let tasks_data = storage.load_tasks()?;

    if tasks_data.is_empty() {
        anyhow::bail!("No tasks found. Run: scud parse-prd <prd-file>");
    }

    // Compute waves for each phase
    let waves_data = compute_all_waves(&tasks_data);

    // Generate HTML
    let html = generate_viewer_html(&tasks_data, &waves_data);

    // Write to temp file
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let temp_file = temp_dir.join(format!("scud-view-{}.html", timestamp));

    fs::write(&temp_file, html)?;

    println!("{}", "Opening SCUD viewer...".green());
    webbrowser::open(temp_file.to_str().unwrap())?;

    Ok(())
}

/// Compute execution waves for all phases
fn compute_all_waves(tasks_data: &HashMap<String, Phase>) -> HashMap<String, Vec<Wave>> {
    let mut waves_data = HashMap::new();

    for (phase_name, phase) in tasks_data {
        let waves = compute_waves(&phase.tasks);
        waves_data.insert(phase_name.clone(), waves);
    }

    waves_data
}

/// Compute execution waves using Kahn's algorithm (topological sort with level assignment)
fn compute_waves(tasks: &[Task]) -> Vec<Wave> {
    // Filter to actionable tasks only (not done, not expanded parents, not cancelled)
    let actionable: Vec<&Task> = tasks
        .iter()
        .filter(|task| {
            if task.status == TaskStatus::Done
                || task.status == TaskStatus::Expanded
                || task.status == TaskStatus::Cancelled
            {
                return false;
            }

            // If it's a subtask, only include if parent is expanded
            if let Some(ref parent_id) = task.parent_id {
                let parent_expanded = tasks
                    .iter()
                    .find(|t| &t.id == parent_id)
                    .map(|p| p.is_expanded())
                    .unwrap_or(false);
                if !parent_expanded {
                    return false;
                }
            }

            true
        })
        .collect();

    if actionable.is_empty() {
        return vec![];
    }

    // Build set of actionable task IDs
    let task_ids: HashSet<String> = actionable.iter().map(|t| t.id.clone()).collect();

    // Build in-degree map (how many dependencies does each task have within our set?)
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for task in &actionable {
        in_degree.entry(task.id.clone()).or_insert(0);

        for dep in &task.dependencies {
            // Only count dependencies that are in our actionable task set
            if task_ids.contains(dep) {
                *in_degree.entry(task.id.clone()).or_insert(0) += 1;
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(task.id.clone());
            }
        }
    }

    // Kahn's algorithm with wave tracking
    let mut waves: Vec<Wave> = Vec::new();
    let mut remaining = in_degree.clone();
    let mut wave_number = 1;

    while !remaining.is_empty() {
        // Find all tasks with no remaining dependencies (in-degree = 0)
        let ready: Vec<String> = remaining
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        if ready.is_empty() {
            // Circular dependency detected - break out
            eprintln!("Warning: Circular dependency detected in tasks");
            break;
        }

        // Remove ready tasks from remaining and update dependents
        for task_id in &ready {
            remaining.remove(task_id);

            if let Some(deps) = dependents.get(task_id) {
                for dep_id in deps {
                    if let Some(deg) = remaining.get_mut(dep_id) {
                        *deg = deg.saturating_sub(1);
                    }
                }
            }
        }

        // Create wave with task details
        let wave_tasks: Vec<WaveTask> = ready
            .iter()
            .filter_map(|task_id| {
                actionable
                    .iter()
                    .find(|t| &t.id == task_id)
                    .map(|task| WaveTask {
                        id: task_id.clone(),
                        title: task.title.clone(),
                        status: task.status.as_str().to_string(),
                        complexity: task.complexity,
                        dependencies: task.dependencies.clone(),
                    })
            })
            .collect();

        waves.push(Wave {
            wave: wave_number,
            round: 1,
            tasks: wave_tasks,
        });

        wave_number += 1;
    }

    waves
}

/// Generate the complete HTML viewer
fn generate_viewer_html(
    tasks_data: &HashMap<String, Phase>,
    waves_data: &HashMap<String, Vec<Wave>>,
) -> String {
    // Convert tasks to JSON-serializable format
    let tasks_json = serde_json::to_string_pretty(tasks_data).unwrap_or_default();
    let waves_json = serde_json::to_string_pretty(waves_data).unwrap_or_default();

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>SCUD Task Viewer</title>
  <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
  <style>
{styles}
  </style>
</head>
<body>
  <header>
    <h1>SCUD Task Viewer</h1>
    <nav>
      <button class="tab-btn active" data-tab="tasks">Tasks</button>
      <button class="tab-btn" data-tab="waves">Waves</button>
      <button class="tab-btn" data-tab="diagram">Diagram</button>
      <button class="tab-btn" data-tab="stats">Stats</button>
    </nav>
  </header>

  <div class="layout">
    <main>
      <section id="tasks" class="tab-content active">
        <div class="phase-selector">
          <label>Phase: </label>
          <select id="phase-select"></select>
        </div>
        <div id="task-list"></div>
      </section>

      <section id="waves" class="tab-content">
        <div class="phase-selector">
          <label>Phase: </label>
          <select id="waves-phase-select"></select>
        </div>
        <div id="waves-list"></div>
      </section>

      <section id="diagram" class="tab-content">
        <div class="diagram-controls">
          <label class="checkbox-label">
            <input type="checkbox" id="simplified-view" checked> Simplified (hide subtasks)
          </label>
        </div>
        <div id="diagram-phases-container"></div>
      </section>

      <section id="stats" class="tab-content">
        <div id="stats-content"></div>
      </section>
    </main>

    <aside id="detail-panel" class="detail-panel hidden">
      <div class="detail-header">
        <h2 id="detail-title">Task Details</h2>
        <button id="close-detail" class="close-btn">&times;</button>
      </div>
      <div id="detail-content"></div>
    </aside>
  </div>

  <script>
    const TASKS_DATA = {tasks_json};
    const WAVES_DATA = {waves_json};
{script}
  </script>
</body>
</html>"#,
        styles = VIEWER_STYLES,
        tasks_json = tasks_json,
        waves_json = waves_json,
        script = VIEWER_SCRIPT
    )
}
