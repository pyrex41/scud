//! Transcript database operations - insert and query Claude Code transcripts.

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::commands::swarm::transcript::{MessageContent, Transcript};

#[derive(Debug)]
pub struct TranscriptSearchResult {
    pub session_id: String,
    pub task_id: Option<String>,
    pub timestamp: String,
    pub role: String,
    pub content_preview: String,
}

#[derive(Debug)]
pub struct TranscriptStats {
    pub total_messages: i64,
    pub total_tool_calls: i64,
    pub total_sessions: i64,
}

/// Insert a full transcript (messages, tool calls, tool results) into the database.
pub fn insert_transcript(
    conn: &Connection,
    transcript: &Transcript,
    scud_session_id: Option<&str>,
    task_id: Option<&str>,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;

    for msg in &transcript.messages {
        let content = match &msg.content {
            MessageContent::Text(t) => t.clone(),
            MessageContent::Structured(s) => serde_json::to_string(s)?,
        };

        // Extract model and token counts from structured content
        let (model, input_tokens, output_tokens) = match &msg.content {
            MessageContent::Structured(s) => (
                s.model.clone(),
                s.usage.as_ref().and_then(|u| u.input_tokens).map(|t| t as i64),
                s.usage.as_ref().and_then(|u| u.output_tokens).map(|t| t as i64),
            ),
            _ => (None, None, None),
        };

        tx.execute(
            "INSERT INTO transcript_messages
             (claude_session_id, scud_session_id, task_id, timestamp, uuid, parent_uuid,
              role, content, model, input_tokens, output_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                transcript.session_id,
                scud_session_id,
                task_id,
                msg.timestamp.to_rfc3339(),
                msg.uuid,
                msg.parent_uuid,
                msg.role,
                content,
                model,
                input_tokens,
                output_tokens,
            ],
        )?;
        let message_id = tx.last_insert_rowid();

        // Insert tool calls that belong to this message (matched by timestamp)
        for tool_call in &transcript.tool_calls {
            if tool_call.timestamp == msg.timestamp {
                tx.execute(
                    "INSERT INTO tool_calls
                     (message_id, claude_session_id, timestamp, tool_id, tool_name, input_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        message_id,
                        transcript.session_id,
                        tool_call.timestamp.to_rfc3339(),
                        tool_call.id,
                        tool_call.name,
                        tool_call.input.to_string(),
                    ],
                )?;
            }
        }

        // Insert tool results that belong to this message
        for tool_result in &transcript.tool_results {
            if tool_result.timestamp == msg.timestamp {
                tx.execute(
                    "INSERT INTO tool_results
                     (message_id, claude_session_id, timestamp, tool_use_id, content, is_error)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        message_id,
                        transcript.session_id,
                        tool_result.timestamp.to_rfc3339(),
                        tool_result.tool_use_id,
                        tool_result.content,
                        tool_result.is_error as i32,
                    ],
                )?;
            }
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn search_transcripts(
    conn: &Connection,
    query: &str,
) -> Result<Vec<TranscriptSearchResult>> {
    let mut stmt = conn.prepare(
        "SELECT tm.claude_session_id, tm.task_id, tm.timestamp, tm.role,
                substr(tm.content, 1, 200) as content_preview
         FROM transcript_messages tm
         WHERE tm.content LIKE ?1
         ORDER BY tm.timestamp DESC
         LIMIT 100",
    )?;

    let pattern = format!("%{}%", query);
    let results = stmt.query_map(params![pattern], |row| {
        Ok(TranscriptSearchResult {
            session_id: row.get(0)?,
            task_id: row.get(1)?,
            timestamp: row.get(2)?,
            role: row.get(3)?,
            content_preview: row.get(4)?,
        })
    })?;

    results.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_transcript_stats(conn: &Connection) -> Result<TranscriptStats> {
    let total_messages: i64 =
        conn.query_row("SELECT COUNT(*) FROM transcript_messages", [], |r| r.get(0))?;
    let total_tool_calls: i64 =
        conn.query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))?;
    let total_sessions: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT claude_session_id) FROM transcript_messages",
        [],
        |r| r.get(0),
    )?;

    Ok(TranscriptStats {
        total_messages,
        total_tool_calls,
        total_sessions,
    })
}

/// List transcript sessions with basic info
pub fn list_transcript_sessions(conn: &Connection) -> Result<Vec<TranscriptSessionInfo>> {
    let mut stmt = conn.prepare(
        "SELECT claude_session_id,
                COUNT(*) as msg_count,
                MIN(timestamp) as first_msg,
                MAX(timestamp) as last_msg
         FROM transcript_messages
         GROUP BY claude_session_id
         ORDER BY first_msg DESC
         LIMIT 50",
    )?;

    let results = stmt.query_map([], |row| {
        Ok(TranscriptSessionInfo {
            session_id: row.get(0)?,
            message_count: row.get(1)?,
            first_message: row.get(2)?,
            last_message: row.get(3)?,
        })
    })?;

    results.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[derive(Debug)]
pub struct TranscriptSessionInfo {
    pub session_id: String,
    pub message_count: i64,
    pub first_message: String,
    pub last_message: String,
}
