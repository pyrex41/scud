use anyhow::Result;
use std::path::PathBuf;

use scud_core::weave::{Coordinator, Event, EventLog};
use scud_core::Phase as CorePhase;
use scud_core::Storage as CoreStorage;

/// Resolve the active group tag from scud_core's Storage.
pub(crate) fn resolve_tag(storage: &CoreStorage) -> Result<String> {
    storage
        .get_active_group()?
        .ok_or_else(|| anyhow::anyhow!("No active task group. Use --tag <tag> or run: scud tags <tag>"))
}

/// Build a Coordinator from the active phase's weave state using scud_core types.
pub(crate) fn build_coordinator(
    storage: &CoreStorage,
) -> Result<(Coordinator, String, CorePhase)> {
    let phase_tag = resolve_tag(storage)?;
    let phase = storage.load_group(&phase_tag)?;

    let weave_dir = storage.scud_dir().join("weave");
    let event_log = EventLog::load(&weave_dir.join("events.jsonl"))?;

    let mut coord = Coordinator::new();
    coord.threads = phase.weave_threads.clone();
    coord.roles = phase.roles.clone();
    coord.partitions = phase.partitions.clone();
    for lock in &phase.locks {
        coord
            .active_locks
            .insert(lock.lock_key.clone(), lock.clone());
    }
    coord.event_log = event_log.events;

    Ok((coord, phase_tag, phase))
}

/// Create a CoreStorage from project_root.
pub(crate) fn core_storage(project_root: Option<PathBuf>) -> CoreStorage {
    CoreStorage::new(project_root)
}

pub fn run(project_root: Option<PathBuf>, event_json: &str) -> Result<()> {
    let storage = core_storage(project_root);
    let event: Event = serde_json::from_str(event_json)
        .map_err(|e| anyhow::anyhow!("Invalid event JSON: {}", e))?;

    let (coord, _tag, _phase) = build_coordinator(&storage)?;
    let decision = coord.evaluate(&event);
    println!("{}", decision);
    Ok(())
}
