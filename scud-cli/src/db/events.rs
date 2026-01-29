//! Event database operations - insert and query swarm events.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::commands::swarm::events::{AgentEvent, EventKind};

pub fn insert_event(conn: &Connection, event: &AgentEvent) -> Result<i64> {
    let (kind, success, duration_ms, tool_name, file_path, dependency_id, reason, data) =
        match &event.event {
            EventKind::Spawned => ("spawned", None, None, None, None, None, None, None),
            EventKind::Started => ("started", None, None, None, None, None, None, None),
            EventKind::Completed {
                success,
                duration_ms,
            } => (
                "completed",
                Some(*success as i32),
                Some(*duration_ms as i64),
                None,
                None,
                None,
                None,
                None,
            ),
            EventKind::Failed { reason } => (
                "failed",
                Some(0),
                None,
                None,
                None,
                None,
                Some(reason.as_str()),
                None,
            ),
            EventKind::ToolCall {
                tool,
                input_summary,
            } => (
                "tool_call",
                None,
                None,
                Some(tool.as_str()),
                None,
                None,
                None,
                input_summary
                    .as_ref()
                    .map(|s| serde_json::json!({"input_summary": s}).to_string()),
            ),
            EventKind::ToolResult {
                tool,
                success,
                duration_ms,
            } => (
                "tool_result",
                Some(*success as i32),
                duration_ms.map(|d| d as i64),
                Some(tool.as_str()),
                None,
                None,
                None,
                None,
            ),
            EventKind::FileRead { path } => (
                "file_read",
                None,
                None,
                None,
                Some(path.as_str()),
                None,
                None,
                None,
            ),
            EventKind::FileWrite {
                path,
                lines_changed,
            } => (
                "file_write",
                None,
                None,
                None,
                Some(path.as_str()),
                None,
                None,
                lines_changed.map(|l| serde_json::json!({"lines_changed": l}).to_string()),
            ),
            EventKind::DependencyMet { dependency_id } => (
                "dependency_met",
                None,
                None,
                None,
                None,
                Some(dependency_id.as_str()),
                None,
                None,
            ),
            EventKind::Unblocked { by_task_id } => (
                "unblocked",
                None,
                None,
                None,
                None,
                Some(by_task_id.as_str()),
                None,
                None,
            ),
            EventKind::Output { line } => (
                "output",
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({"line": line}).to_string()),
            ),
            EventKind::WaveStarted {
                wave_number,
                task_count,
            } => (
                "wave_started",
                None,
                None,
                None,
                None,
                None,
                None,
                Some(
                    serde_json::json!({"wave_number": wave_number, "task_count": task_count})
                        .to_string(),
                ),
            ),
            EventKind::WaveCompleted {
                wave_number,
                duration_ms,
            } => (
                "wave_completed",
                None,
                Some(*duration_ms as i64),
                None,
                None,
                None,
                None,
                Some(serde_json::json!({"wave_number": wave_number}).to_string()),
            ),
            EventKind::ValidationPassed => (
                "validation_passed",
                Some(1),
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            EventKind::ValidationFailed { failures } => (
                "validation_failed",
                Some(0),
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({"failures": failures}).to_string()),
            ),
            EventKind::RepairStarted { attempt, task_ids } => (
                "repair_started",
                None,
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({"attempt": attempt, "task_ids": task_ids}).to_string()),
            ),
            EventKind::RepairCompleted { attempt, success } => (
                "repair_completed",
                Some(*success as i32),
                None,
                None,
                None,
                None,
                None,
                Some(serde_json::json!({"attempt": attempt}).to_string()),
            ),
            EventKind::Heartbeat => ("heartbeat", None, None, None, None, None, None, None),
            EventKind::Custom { name, data } => (
                "custom",
                None,
                None,
                None,
                None,
                None,
                Some(name.as_str()),
                data.as_ref().map(|d| d.to_string()),
            ),
        };

    conn.execute(
        "INSERT INTO events (timestamp, session_id, task_id, kind, success, duration_ms,
         tool_name, file_path, dependency_id, reason, data)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            event.timestamp.to_rfc3339(),
            event.session_id,
            event.task_id,
            kind,
            success,
            duration_ms,
            tool_name,
            file_path,
            dependency_id,
            reason,
            data,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_events_for_session(conn: &Connection, session_id: &str) -> Result<Vec<AgentEvent>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, session_id, task_id, kind, success, duration_ms,
                tool_name, file_path, dependency_id, reason, data
         FROM events WHERE session_id = ? ORDER BY timestamp ASC",
    )?;

    let events = stmt.query_map(params![session_id], |row| {
        let timestamp: String = row.get(0)?;
        let session_id: String = row.get(1)?;
        let task_id: String = row.get(2)?;
        let kind: String = row.get(3)?;
        let success: Option<i32> = row.get(4)?;
        let duration_ms: Option<i64> = row.get(5)?;
        let tool_name: Option<String> = row.get(6)?;
        let file_path: Option<String> = row.get(7)?;
        let dependency_id: Option<String> = row.get(8)?;
        let reason: Option<String> = row.get(9)?;
        let data: Option<String> = row.get(10)?;

        let event = match kind.as_str() {
            "spawned" => EventKind::Spawned,
            "started" => EventKind::Started,
            "completed" => EventKind::Completed {
                success: success.unwrap_or(0) != 0,
                duration_ms: duration_ms.unwrap_or(0) as u64,
            },
            "failed" => EventKind::Failed {
                reason: reason.unwrap_or_default(),
            },
            "tool_call" => EventKind::ToolCall {
                tool: tool_name.clone().unwrap_or_default(),
                input_summary: data.as_ref().and_then(|d| {
                    serde_json::from_str::<serde_json::Value>(d)
                        .ok()
                        .and_then(|v| {
                            v.get("input_summary")
                                .and_then(|s| s.as_str())
                                .map(String::from)
                        })
                }),
            },
            "tool_result" => EventKind::ToolResult {
                tool: tool_name.clone().unwrap_or_default(),
                success: success.unwrap_or(0) != 0,
                duration_ms: duration_ms.map(|d| d as u64),
            },
            "file_read" => EventKind::FileRead {
                path: file_path.clone().unwrap_or_default(),
            },
            "file_write" => EventKind::FileWrite {
                path: file_path.clone().unwrap_or_default(),
                lines_changed: data.as_ref().and_then(|d| {
                    serde_json::from_str::<serde_json::Value>(d)
                        .ok()
                        .and_then(|v| v.get("lines_changed").and_then(|n| n.as_u64()))
                        .map(|n| n as u32)
                }),
            },
            "dependency_met" => EventKind::DependencyMet {
                dependency_id: dependency_id.clone().unwrap_or_default(),
            },
            "unblocked" => EventKind::Unblocked {
                by_task_id: dependency_id.clone().unwrap_or_default(),
            },
            "output" => EventKind::Output {
                line: data
                    .as_ref()
                    .and_then(|d| {
                        serde_json::from_str::<serde_json::Value>(d)
                            .ok()
                            .and_then(|v| v.get("line").and_then(|s| s.as_str()).map(String::from))
                    })
                    .unwrap_or_default(),
            },
            "wave_started" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::WaveStarted {
                    wave_number: parsed
                        .as_ref()
                        .and_then(|v| v.get("wave_number").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    task_count: parsed
                        .as_ref()
                        .and_then(|v| v.get("task_count").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                }
            }
            "wave_completed" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::WaveCompleted {
                    wave_number: parsed
                        .as_ref()
                        .and_then(|v| v.get("wave_number").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    duration_ms: duration_ms.unwrap_or(0) as u64,
                }
            }
            "validation_passed" => EventKind::ValidationPassed,
            "validation_failed" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::ValidationFailed {
                    failures: parsed
                        .as_ref()
                        .and_then(|v| v.get("failures"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            }
            "repair_started" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::RepairStarted {
                    attempt: parsed
                        .as_ref()
                        .and_then(|v| v.get("attempt").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    task_ids: parsed
                        .as_ref()
                        .and_then(|v| v.get("task_ids"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            }
            "repair_completed" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::RepairCompleted {
                    attempt: parsed
                        .as_ref()
                        .and_then(|v| v.get("attempt").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    success: success.unwrap_or(0) != 0,
                }
            }
            _ => EventKind::Custom {
                name: kind.clone(),
                data: data.and_then(|d| serde_json::from_str(&d).ok()),
            },
        };

        Ok(AgentEvent {
            timestamp: DateTime::parse_from_rfc3339(&timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            session_id,
            task_id,
            event,
        })
    })?;

    events.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_events_for_session_limited(
    conn: &Connection,
    session_id: &str,
    limit: Option<usize>,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<AgentEvent>> {
    let mut query = "SELECT timestamp, session_id, task_id, kind, success, duration_ms,
                    tool_name, file_path, dependency_id, reason, data
             FROM events WHERE session_id = ?"
        .to_string();
    let mut params: Vec<String> = vec![session_id.to_string()];

    if let Some(since) = since {
        query.push_str(" AND timestamp >= ?");
        params.push(since.to_rfc3339());
    }

    query.push_str(" ORDER BY timestamp DESC");

    if let Some(limit) = limit {
        query.push_str(&format!(" LIMIT {}", limit));
    }

    let mut stmt = conn.prepare(&query)?;

    let events = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        let timestamp: String = row.get(0)?;
        let session_id: String = row.get(1)?;
        let task_id: String = row.get(2)?;
        let kind: String = row.get(3)?;
        let success: Option<i32> = row.get(4)?;
        let duration_ms: Option<i64> = row.get(5)?;
        let tool_name: Option<String> = row.get(6)?;
        let file_path: Option<String> = row.get(7)?;
        let dependency_id: Option<String> = row.get(8)?;
        let reason: Option<String> = row.get(9)?;
        let data: Option<String> = row.get(10)?;

        let event = match kind.as_str() {
            "spawned" => EventKind::Spawned,
            "started" => EventKind::Started,
            "completed" => EventKind::Completed {
                success: success.unwrap_or(0) != 0,
                duration_ms: duration_ms.unwrap_or(0) as u64,
            },
            "failed" => EventKind::Failed {
                reason: reason.unwrap_or_default(),
            },
            "tool_call" => EventKind::ToolCall {
                tool: tool_name.clone().unwrap_or_default(),
                input_summary: data.as_ref().and_then(|d| {
                    serde_json::from_str::<serde_json::Value>(d)
                        .ok()
                        .and_then(|v| {
                            v.get("input_summary")
                                .and_then(|s| s.as_str())
                                .map(String::from)
                        })
                }),
            },
            "tool_result" => EventKind::ToolResult {
                tool: tool_name.clone().unwrap_or_default(),
                success: success.unwrap_or(0) != 0,
                duration_ms: duration_ms.map(|d| d as u64),
            },
            "file_read" => EventKind::FileRead {
                path: file_path.clone().unwrap_or_default(),
            },
            "file_write" => EventKind::FileWrite {
                path: file_path.clone().unwrap_or_default(),
                lines_changed: data.as_ref().and_then(|d| {
                    serde_json::from_str::<serde_json::Value>(d)
                        .ok()
                        .and_then(|v| v.get("lines_changed").and_then(|n| n.as_u64()))
                        .map(|n| n as u32)
                }),
            },
            "dependency_met" => EventKind::DependencyMet {
                dependency_id: dependency_id.clone().unwrap_or_default(),
            },
            "unblocked" => EventKind::Unblocked {
                by_task_id: dependency_id.clone().unwrap_or_default(),
            },
            "output" => EventKind::Output {
                line: data
                    .as_ref()
                    .and_then(|d| {
                        serde_json::from_str::<serde_json::Value>(d)
                            .ok()
                            .and_then(|v| v.get("line").and_then(|s| s.as_str()).map(String::from))
                    })
                    .unwrap_or_default(),
            },
            "wave_started" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::WaveStarted {
                    wave_number: parsed
                        .as_ref()
                        .and_then(|v| v.get("wave_number").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    task_count: parsed
                        .as_ref()
                        .and_then(|v| v.get("task_count").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                }
            }
            "wave_completed" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::WaveCompleted {
                    wave_number: parsed
                        .as_ref()
                        .and_then(|v| v.get("wave_number").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    duration_ms: duration_ms.unwrap_or(0) as u64,
                }
            }
            "validation_passed" => EventKind::ValidationPassed,
            "validation_failed" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::ValidationFailed {
                    failures: parsed
                        .as_ref()
                        .and_then(|v| v.get("failures"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            }
            "repair_started" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::RepairStarted {
                    attempt: parsed
                        .as_ref()
                        .and_then(|v| v.get("attempt").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    task_ids: parsed
                        .as_ref()
                        .and_then(|v| v.get("task_ids"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            }
            "repair_completed" => {
                let parsed = data
                    .as_ref()
                    .and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok());
                EventKind::RepairCompleted {
                    attempt: parsed
                        .as_ref()
                        .and_then(|v| v.get("attempt").and_then(|n| n.as_u64()))
                        .unwrap_or(0) as usize,
                    success: success.unwrap_or(0) != 0,
                }
            }
            _ => EventKind::Custom {
                name: kind.clone(),
                data: data.and_then(|d| serde_json::from_str(&d).ok()),
            },
        };

        Ok(AgentEvent {
            timestamp: DateTime::parse_from_rfc3339(&timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            session_id,
            task_id,
            event,
        })
    })?;

    let mut events: Vec<AgentEvent> = events.collect::<Result<Vec<_>, _>>()?;
    events.reverse(); // Reverse to chronological order since we queried DESC
    Ok(events)
}

pub fn list_sessions(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, MIN(timestamp) as first_ts
         FROM events GROUP BY session_id ORDER BY first_ts ASC",
    )?;
    let sessions = stmt.query_map([], |row| row.get::<_, String>(0))?;
    sessions.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
