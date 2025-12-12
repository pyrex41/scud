use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::models::{Phase, Priority, TaskStatus};
use crate::storage::Storage;

struct AppState {
    storage: Storage,
}

pub async fn run(project_root: Option<PathBuf>, port: u16, no_open: bool) -> Result<()> {
    let storage = Storage::new(project_root);

    // Verify we have tasks
    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    let state = Arc::new(AppState { storage });

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/tasks", get(tasks_json_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let url = format!("http://{}", addr);

    println!("Starting SCUD web server at {}", url);

    // Open browser unless --no-open
    if !no_open {
        if let Err(e) = open::that(&url) {
            eprintln!("Failed to open browser: {}", e);
        }
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match generate_html(&state.storage) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn tasks_json_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.storage.load_tasks() {
        Ok(tasks) => {
            let json = serde_json::to_string_pretty(&tasks).unwrap_or_default();
            (StatusCode::OK, [("content-type", "application/json")], json).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn generate_html(storage: &Storage) -> Result<String> {
    let all_tasks = storage.load_tasks()?;
    let active_tag = storage.get_active_group()?.unwrap_or_default();

    // Generate task rows HTML
    let mut task_rows = String::new();
    let mut phase_options = String::new();

    let mut sorted_tags: Vec<_> = all_tasks.keys().collect();
    sorted_tags.sort();

    for tag in &sorted_tags {
        let selected = if *tag == &active_tag { "selected" } else { "" };
        phase_options.push_str(&format!(
            r#"<option value="{}" {}>{}</option>"#,
            tag, selected, tag
        ));
    }

    // Get active phase or first available
    let display_tag = if !active_tag.is_empty() && all_tasks.contains_key(&active_tag) {
        active_tag.clone()
    } else {
        sorted_tags.first().map(|s| s.to_string()).unwrap_or_default()
    };

    if let Some(phase) = all_tasks.get(&display_tag) {
        task_rows = generate_task_rows(phase);
    }

    // Generate Mermaid diagram
    let mermaid_diagram = generate_mermaid(&all_tasks, &display_tag);

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SCUD - Task Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
    <style>
        :root {{
            --bg: #0d1117;
            --bg-secondary: #161b22;
            --border: #30363d;
            --text: #c9d1d9;
            --text-muted: #8b949e;
            --accent: #58a6ff;
            --green: #3fb950;
            --yellow: #d29922;
            --red: #f85149;
            --purple: #a371f7;
            --cyan: #39c5cf;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
            background: var(--bg);
            color: var(--text);
            line-height: 1.5;
        }}
        .container {{
            max-width: 1400px;
            margin: 0 auto;
            padding: 20px;
        }}
        header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 16px 0;
            border-bottom: 1px solid var(--border);
            margin-bottom: 24px;
        }}
        h1 {{
            font-size: 24px;
            font-weight: 600;
            display: flex;
            align-items: center;
            gap: 8px;
        }}
        .controls {{
            display: flex;
            gap: 12px;
            align-items: center;
        }}
        select, input {{
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            color: var(--text);
            padding: 8px 12px;
            border-radius: 6px;
            font-size: 14px;
        }}
        select:focus, input:focus {{
            outline: none;
            border-color: var(--accent);
        }}
        .grid {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 24px;
        }}
        @media (max-width: 1000px) {{
            .grid {{ grid-template-columns: 1fr; }}
        }}
        .panel {{
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 8px;
            overflow: hidden;
        }}
        .panel-header {{
            padding: 12px 16px;
            border-bottom: 1px solid var(--border);
            font-weight: 600;
            font-size: 14px;
        }}
        .panel-body {{
            padding: 16px;
            max-height: 600px;
            overflow-y: auto;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            font-size: 13px;
        }}
        th {{
            text-align: left;
            padding: 8px;
            color: var(--text-muted);
            font-weight: 500;
            border-bottom: 1px solid var(--border);
        }}
        td {{
            padding: 8px;
            border-bottom: 1px solid var(--border);
        }}
        tr:hover {{
            background: rgba(255,255,255,0.02);
        }}
        .status {{
            display: inline-flex;
            align-items: center;
            gap: 4px;
            padding: 2px 8px;
            border-radius: 12px;
            font-size: 12px;
            font-weight: 500;
        }}
        .status-pending {{ background: rgba(139,148,158,0.2); color: var(--text-muted); }}
        .status-in-progress {{ background: rgba(210,153,34,0.2); color: var(--yellow); }}
        .status-done {{ background: rgba(63,185,80,0.2); color: var(--green); }}
        .status-blocked {{ background: rgba(248,81,73,0.2); color: var(--red); }}
        .status-review {{ background: rgba(57,197,207,0.2); color: var(--cyan); }}
        .status-expanded {{ background: rgba(163,113,247,0.2); color: var(--purple); }}
        .status-deferred {{ background: rgba(139,148,158,0.15); color: var(--text-muted); }}
        .status-cancelled {{ background: rgba(139,148,158,0.1); color: var(--text-muted); text-decoration: line-through; }}
        .priority-critical {{ color: var(--red); font-weight: 600; }}
        .priority-high {{ color: var(--yellow); }}
        .priority-medium {{ color: var(--text); }}
        .priority-low {{ color: var(--text-muted); }}
        .task-id {{
            font-family: monospace;
            color: var(--accent);
        }}
        .task-title {{
            max-width: 300px;
            overflow: hidden;
            text-overflow: ellipsis;
            white-space: nowrap;
        }}
        .subtask {{
            padding-left: 20px;
            color: var(--text-muted);
        }}
        #mermaid-container {{
            background: var(--bg);
            border-radius: 4px;
            min-height: 400px;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        .mermaid {{
            width: 100%;
        }}
        .stats {{
            display: flex;
            gap: 24px;
            margin-bottom: 24px;
        }}
        .stat {{
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 16px 24px;
            text-align: center;
        }}
        .stat-value {{
            font-size: 32px;
            font-weight: 600;
        }}
        .stat-label {{
            font-size: 12px;
            color: var(--text-muted);
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}
        .stat-done .stat-value {{ color: var(--green); }}
        .stat-progress .stat-value {{ color: var(--yellow); }}
        .stat-pending .stat-value {{ color: var(--text-muted); }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>SCUD Task Dashboard</h1>
            <div class="controls">
                <select id="phase-select" onchange="location.reload()">
                    {phase_options}
                </select>
                <input type="text" id="search" placeholder="Filter tasks..." oninput="filterTasks()">
            </div>
        </header>

        <div class="stats" id="stats"></div>

        <div class="grid">
            <div class="panel">
                <div class="panel-header">Tasks</div>
                <div class="panel-body">
                    <table>
                        <thead>
                            <tr>
                                <th>ID</th>
                                <th>Title</th>
                                <th>Status</th>
                                <th>Priority</th>
                                <th>Cplx</th>
                            </tr>
                        </thead>
                        <tbody id="task-list">
                            {task_rows}
                        </tbody>
                    </table>
                </div>
            </div>
            <div class="panel">
                <div class="panel-header">Dependency Graph</div>
                <div class="panel-body" id="mermaid-container">
                    <pre class="mermaid">
{mermaid_diagram}
                    </pre>
                </div>
            </div>
        </div>
    </div>

    <script>
        mermaid.initialize({{
            startOnLoad: true,
            theme: 'dark',
            flowchart: {{
                useMaxWidth: true,
                htmlLabels: true,
                curve: 'basis'
            }}
        }});

        function filterTasks() {{
            const search = document.getElementById('search').value.toLowerCase();
            const rows = document.querySelectorAll('#task-list tr');
            rows.forEach(row => {{
                const text = row.textContent.toLowerCase();
                row.style.display = text.includes(search) ? '' : 'none';
            }});
        }}

        // Calculate stats
        document.addEventListener('DOMContentLoaded', () => {{
            const rows = document.querySelectorAll('#task-list tr');
            let done = 0, progress = 0, pending = 0, total = rows.length;
            rows.forEach(row => {{
                const status = row.querySelector('.status')?.textContent.toLowerCase() || '';
                if (status.includes('done')) done++;
                else if (status.includes('progress')) progress++;
                else pending++;
            }});
            const pct = total > 0 ? Math.round(done / total * 100) : 0;
            document.getElementById('stats').innerHTML = `
                <div class="stat stat-done"><div class="stat-value">${{done}}</div><div class="stat-label">Done</div></div>
                <div class="stat stat-progress"><div class="stat-value">${{progress}}</div><div class="stat-label">In Progress</div></div>
                <div class="stat stat-pending"><div class="stat-value">${{pending}}</div><div class="stat-label">Pending</div></div>
                <div class="stat"><div class="stat-value">${{pct}}%</div><div class="stat-label">Complete</div></div>
            `;
        }});
    </script>
</body>
</html>
"##,
        phase_options = phase_options,
        task_rows = task_rows,
        mermaid_diagram = mermaid_diagram,
    );

    Ok(html)
}

fn generate_task_rows(phase: &Phase) -> String {
    let mut rows = String::new();
    let mut sorted_tasks = phase.tasks.clone();
    sorted_tasks.sort_by(|a, b| natural_sort_ids(&a.id, &b.id));

    for task in &sorted_tasks {
        let status_class = match task.status {
            TaskStatus::Pending => "status-pending",
            TaskStatus::InProgress => "status-in-progress",
            TaskStatus::Done => "status-done",
            TaskStatus::Blocked => "status-blocked",
            TaskStatus::Review => "status-review",
            TaskStatus::Expanded => "status-expanded",
            TaskStatus::Deferred => "status-deferred",
            TaskStatus::Cancelled => "status-cancelled",
        };

        let status_text = match task.status {
            TaskStatus::Pending => "Pending",
            TaskStatus::InProgress => "In Progress",
            TaskStatus::Done => "Done",
            TaskStatus::Blocked => "Blocked",
            TaskStatus::Review => "Review",
            TaskStatus::Expanded => "Expanded",
            TaskStatus::Deferred => "Deferred",
            TaskStatus::Cancelled => "Cancelled",
        };

        let priority_class = match task.priority {
            Priority::Critical => "priority-critical",
            Priority::High => "priority-high",
            Priority::Medium => "priority-medium",
            Priority::Low => "priority-low",
        };

        let priority_text = match task.priority {
            Priority::Critical => "Critical",
            Priority::High => "High",
            Priority::Medium => "Medium",
            Priority::Low => "Low",
        };

        let is_subtask = task.parent_id.is_some();
        let title_class = if is_subtask { "task-title subtask" } else { "task-title" };

        rows.push_str(&format!(
            r#"<tr data-id="{}">
                <td class="task-id">{}</td>
                <td class="{}" title="{}">{}</td>
                <td><span class="status {}">{}</span></td>
                <td class="{}">{}</td>
                <td>{}</td>
            </tr>"#,
            task.id,
            task.id,
            title_class,
            html_escape(&task.title),
            html_escape(&truncate(&task.title, 40)),
            status_class,
            status_text,
            priority_class,
            priority_text,
            task.complexity,
        ));
    }

    rows
}

fn generate_mermaid(all_tasks: &std::collections::HashMap<String, Phase>, tag: &str) -> String {
    let mut output = String::from("flowchart TD\n");

    if let Some(phase) = all_tasks.get(tag) {
        // Add task nodes
        for task in &phase.tasks {
            let node_id = sanitize_id(&task.id);
            let label = escape_label(&task.title);
            let status_class = match task.status {
                TaskStatus::Pending => "pending",
                TaskStatus::InProgress => "inprogress",
                TaskStatus::Done => "done",
                TaskStatus::Blocked => "blocked",
                TaskStatus::Review => "review",
                TaskStatus::Expanded => "expanded",
                TaskStatus::Deferred => "deferred",
                TaskStatus::Cancelled => "cancelled",
            };

            // Format based on status
            let shape = match task.status {
                TaskStatus::Expanded => format!("    {}[[\"{}\"]]", node_id, label),
                TaskStatus::Done => format!("    {}([\"{}\"])", node_id, label),
                TaskStatus::Blocked => format!("    {}{{\"{}\"}}", node_id, label),
                _ => format!("    {}[\"{}\"]", node_id, label),
            };

            output.push_str(&shape);
            output.push('\n');
            output.push_str(&format!("    class {} {}\n", node_id, status_class));
        }

        // Add dependency edges
        for task in &phase.tasks {
            let task_node = sanitize_id(&task.id);
            for dep in &task.dependencies {
                let dep_node = sanitize_id(dep);
                output.push_str(&format!("    {} --> {}\n", dep_node, task_node));
            }

            // Parent-child relationships
            if let Some(ref parent_id) = task.parent_id {
                let parent_node = sanitize_id(parent_id);
                output.push_str(&format!("    {} -.-> {}\n", parent_node, task_node));
            }
        }

        // Add styles
        output.push_str("\n    %% Status styles\n");
        output.push_str("    classDef pending fill:#21262d,stroke:#30363d,color:#8b949e\n");
        output.push_str("    classDef inprogress fill:#2d2a1e,stroke:#d29922,color:#d29922,stroke-width:2px\n");
        output.push_str("    classDef done fill:#1b2d22,stroke:#3fb950,color:#3fb950\n");
        output.push_str("    classDef blocked fill:#2d1b1e,stroke:#f85149,color:#f85149\n");
        output.push_str("    classDef review fill:#1b2d2d,stroke:#39c5cf,color:#39c5cf\n");
        output.push_str("    classDef expanded fill:#251b2d,stroke:#a371f7,color:#a371f7\n");
        output.push_str("    classDef deferred fill:#1b1d21,stroke:#484f58,color:#484f58\n");
        output.push_str("    classDef cancelled fill:#161b22,stroke:#30363d,color:#484f58\n");
    }

    output
}

fn sanitize_id(id: &str) -> String {
    id.replace([':', '.', '-', ' '], "_")
}

fn escape_label(text: &str) -> String {
    text.replace('"', "'")
        .replace('\n', " ")
        .chars()
        .take(30)
        .collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

fn natural_sort_ids(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        match (ap.parse::<u32>(), bp.parse::<u32>()) {
            (Ok(an), Ok(bn)) => {
                if an != bn {
                    return an.cmp(&bn);
                }
            }
            _ => {
                if ap != bp {
                    return ap.cmp(bp);
                }
            }
        }
    }
    a_parts.len().cmp(&b_parts.len())
}
