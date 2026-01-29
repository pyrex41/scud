//! Watch command - monitor running swarms via ZMQ

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use zeromq::{Socket, SocketRecv};

use crate::commands::swarm::publisher::{discover_sessions, ZmqEvent};

#[derive(Parser, Debug)]
pub struct WatchArgs {
    /// Session ID to watch (discovers automatically if not specified)
    #[arg(long)]
    pub session: Option<String>,

    /// Tag to filter sessions
    #[arg(long)]
    pub tag: Option<String>,

    /// Project root directory
    #[arg(long)]
    pub project_root: Option<PathBuf>,

    /// Output format: text, json
    #[arg(long, default_value = "text")]
    pub format: String,
}

pub async fn run(args: WatchArgs) -> Result<()> {
    let project_root = args
        .project_root
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    // Discover sessions
    let sessions = discover_sessions(&project_root);

    // Filter sessions based on criteria
    let filtered_sessions: Vec<_> = sessions
        .into_iter()
        .filter(|s| {
            // Filter by tag if specified
            if let Some(ref tag) = args.tag {
                if s.tag != *tag {
                    return false;
                }
            }
            // Filter by session if specified
            if let Some(ref session) = args.session {
                if s.session_id != *session {
                    return false;
                }
            }
            true
        })
        .collect();

    if filtered_sessions.is_empty() {
        println!("No running swarms found.");
        if args.tag.is_some() {
            println!("Try without --tag to see all sessions.");
        }
        return Ok(());
    }

    // If multiple sessions, list them
    if filtered_sessions.len() > 1 && args.session.is_none() {
        println!("Multiple swarms running. Specify --session to watch one:");
        for session in &filtered_sessions {
            println!("  {} (tag: {})", session.session_id, session.tag);
        }
        return Ok(());
    }

    let session = filtered_sessions.into_iter().next().unwrap();
    println!("Watching swarm: {}", session.session_id);
    println!("Connected to: {}", session.pub_endpoint);
    println!("---");

    // Connect and subscribe to ZMQ
    let mut socket = zeromq::SubSocket::new();
    socket.connect(&session.pub_endpoint).await?;
    socket.subscribe("").await?;

    // Receive and print events
    loop {
        match socket.recv().await {
            Ok(msg) => {
                // Get the first frame from the multi-part message
                if let Some(frame) = msg.iter().next() {
                    if let Ok(text) = std::str::from_utf8(&frame) {
                        if args.format == "json" {
                            println!("{}", text);
                        } else if let Ok(event) = serde_json::from_str::<ZmqEvent>(text) {
                            print_event(&event);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Connection lost: {}", e);
                break;
            }
        }
    }

    Ok(())
}

fn print_event(event: &ZmqEvent) {
    match event {
        ZmqEvent::SwarmStarted {
            tag, total_waves, ..
        } => {
            println!("[SWARM] Started tag='{}' waves={}", tag, total_waves);
        }
        ZmqEvent::WaveStarted { wave, tasks, task_count } => {
            println!(
                "[WAVE {}] Started with {} tasks",
                wave,
                task_count
            );
            if !tasks.is_empty() {
                println!("  Tasks: {:?}", tasks);
            }
        }
        ZmqEvent::TaskStarted { task_id } => {
            println!("[TASK {}] Started", task_id);
        }
        ZmqEvent::TaskSpawned { task_id } => {
            println!("[TASK {}] Spawned", task_id);
        }
        ZmqEvent::TaskOutput { task_id, text } => {
            println!("[{}] {}", task_id, text);
        }
        ZmqEvent::TaskCompleted {
            task_id,
            success,
            duration_ms,
        } => {
            let status = if *success { "completed" } else { "FAILED" };
            let duration = duration_ms
                .map(|d| format!(" ({}ms)", d))
                .unwrap_or_default();
            println!("[TASK {}] {}{}", task_id, status, duration);
        }
        ZmqEvent::TaskFailed { task_id, reason } => {
            println!("[TASK {}] FAILED: {}", task_id, reason);
        }
        ZmqEvent::ValidationStarted => {
            println!("[VALIDATION] Running...");
        }
        ZmqEvent::ValidationCompleted { passed, output } => {
            let status = if *passed { "PASSED" } else { "FAILED" };
            println!("[VALIDATION] {}: {}", status, output);
        }
        ZmqEvent::ValidationPassed => {
            println!("[VALIDATION] PASSED");
        }
        ZmqEvent::ValidationFailed { failures } => {
            println!("[VALIDATION] FAILED:");
            for failure in failures {
                println!("  - {}", failure);
            }
        }
        ZmqEvent::WaveCompleted { wave, duration_ms } => {
            let duration = duration_ms
                .map(|d| format!(" ({}ms)", d))
                .unwrap_or_default();
            println!("[WAVE {}] Completed{}", wave, duration);
        }
        ZmqEvent::SwarmCompleted { success } => {
            let status = if *success { "SUCCESS" } else { "FAILED" };
            println!("[SWARM] Completed: {}", status);
        }
        ZmqEvent::SwarmPaused => {
            println!("[SWARM] Paused");
        }
        ZmqEvent::SwarmResumed => {
            println!("[SWARM] Resumed");
        }
        ZmqEvent::Heartbeat { timestamp } => {
            println!("[HEARTBEAT] {}", timestamp);
        }
        ZmqEvent::ToolCall { task_id, tool, input_summary } => {
            let summary = input_summary.as_ref().map(|s| format!(" ({})", s)).unwrap_or_default();
            println!("[TOOL {}] {} called{}", task_id, tool, summary);
        }
        ZmqEvent::ToolResult { task_id, tool, success, duration_ms } => {
            let status = if *success { "success" } else { "failed" };
            let duration = duration_ms.map(|d| format!(" ({}ms)", d)).unwrap_or_default();
            println!("[TOOL {}] {} {}{}", task_id, tool, status, duration);
        }
        ZmqEvent::FileRead { task_id, path } => {
            println!("[FILE {}] Read: {}", task_id, path);
        }
        ZmqEvent::FileWrite { task_id, path, lines_changed } => {
            let lines = lines_changed.map(|l| format!(" ({} lines)", l)).unwrap_or_default();
            println!("[FILE {}] Write: {}{}", task_id, path, lines);
        }
        ZmqEvent::DependencyMet { task_id, dependency_id } => {
            println!("[DEP {}] Met: {}", task_id, dependency_id);
        }
        ZmqEvent::TaskUnblocked { task_id, by_task_id } => {
            println!("[BLOCK {}] Unblocked by: {}", task_id, by_task_id);
        }
        ZmqEvent::RepairStarted { attempt, task_ids } => {
            println!("[REPAIR] Started attempt {} for tasks: {:?}", attempt, task_ids);
        }
        ZmqEvent::RepairCompleted { attempt, success } => {
            let status = if *success { "succeeded" } else { "failed" };
            println!("[REPAIR] Attempt {} {}", attempt, status);
        }
    }
}