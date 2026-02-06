use anyhow::Result;
use std::path::PathBuf;

use scud_core::weave::{Event, EventLog};

use super::check::{build_coordinator, core_storage, resolve_tag};

pub fn run(project_root: Option<PathBuf>, event_json: &str) -> Result<()> {
    let storage = core_storage(project_root);
    let event: Event = serde_json::from_str(event_json)
        .map_err(|e| anyhow::anyhow!("Invalid event JSON: {}", e))?;

    let phase_tag = resolve_tag(&storage)?;
    let (mut coord, _tag, _phase) = build_coordinator(&storage)?;

    // Record the event (updates locks and event log)
    coord.record(&event)?;

    // Save event log back
    let weave_dir = storage.scud_dir().join("weave");
    let event_log = EventLog {
        events: coord.event_log,
    };
    event_log.save(&weave_dir.join("events.jsonl"))?;

    // Update locks in the phase SCG
    let mut phase = storage.load_group(&phase_tag)?;
    phase.locks = coord.active_locks.values().cloned().collect();
    storage.update_group(&phase_tag, &phase)?;

    println!("Recorded: {}", event.kind);
    Ok(())
}
