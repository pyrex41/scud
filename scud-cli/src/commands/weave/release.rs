use anyhow::Result;
use std::path::PathBuf;

use super::check::{build_coordinator, core_storage, resolve_tag};

pub fn run_release(
    project_root: Option<PathBuf>,
    lock_key: &str,
    agent: Option<&str>,
) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let (mut coord, _tag, _phase) = build_coordinator(&storage)?;

    let agent_str = agent.unwrap_or("");
    coord.release(lock_key, agent_str)?;

    // Save updated locks back to phase
    let mut phase = storage.load_group(&phase_tag)?;
    phase.locks = coord.active_locks.values().cloned().collect();
    storage.update_group(&phase_tag, &phase)?;

    println!("Released: {}", lock_key);
    Ok(())
}

pub fn run_release_all(
    project_root: Option<PathBuf>,
    agent: &str,
    task: Option<&str>,
) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let (mut coord, _tag, _phase) = build_coordinator(&storage)?;

    let before = coord.active_locks.len();
    coord.release_all(agent, task)?;
    let released = before - coord.active_locks.len();

    // Save updated locks back to phase
    let mut phase = storage.load_group(&phase_tag)?;
    phase.locks = coord.active_locks.values().cloned().collect();
    storage.update_group(&phase_tag, &phase)?;

    println!("Released {} lock(s) for agent {}", released, agent);
    Ok(())
}
