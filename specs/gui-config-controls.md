# Descartes GUI: Full Configuration Controls

## Job to Be Done
When launching AI swarm sessions from the Descartes GUI, I want to configure harness, model, round size, tag, and agent type from the UI, so I can control swarm behavior without editing config files.

## Functional Requirements

### Phase 1: State
- [ ] FR-1: Add `LaunchConfig` struct with fields: harness (String), model (String), round_size (usize), tag (String), agent_type (Option<String>)
- [ ] FR-2: Add `LaunchConfig::from_defaults(defaults: &SwarmDefaults)` constructor
- [ ] FR-3: Add to `AppState`: launch_config, available_harnesses (static vec), available_tags (loaded), available_agents (loaded)

### Phase 2: Messages + Handlers
- [ ] FR-4: Add Message variants: SetHarness, SetModel, SetRoundSize, SetLaunchTag, SetAgentType, TagsLoaded, AgentsLoaded, SpawnTask
- [ ] FR-5: SpawnTask handler builds RunTaskHeadless from launch_config
- [ ] FR-6: StartSwarmHeadless handler passes launch_config.model
- [ ] FR-7: Init sends LoadAvailableTags and LoadAvailableAgents on startup

### Phase 3: Agents View Rewrite
- [ ] FR-8: Rewrite agents.rs as config panel with pick_lists for harness, round_size, tag, agent
- [ ] FR-9: Add text_input for model field
- [ ] FR-10: Display status (Idle/Running/Paused) and current task
- [ ] FR-11: Start Headless Swarm and Start Swarm (tmux) buttons use launch_config values
- [ ] FR-12: Pause/Resume/Stop control buttons

### Phase 4: Waves View - Spawn Button
- [ ] FR-13: Add "Spawn" button per non-done task in waves view
- [ ] FR-14: Spawn uses current launch_config harness/model settings

### Phase 5: Bridge - Model Plumbing
- [ ] FR-15: Add model field to ScudCommand::StartSwarmHeadless, RunTaskHeadless, RunTask, StartSwarm
- [ ] FR-16: Add ScudCommand::LoadAvailableTags (reads Storage::load_tasks() keys)
- [ ] FR-17: Add ScudCommand::LoadAvailableAgents (reads .scud/agents/ directory)
- [ ] FR-18: Add ScudEvent::TagsLoaded and ScudEvent::AgentsLoaded
- [ ] FR-19: Pass model through to runner.start() calls

### Phase 6: Tests
- [ ] FR-20: Update test_app() helper for new AppState fields
- [ ] FR-21: Update test_ui_swarm_controls for new agents view signature
- [ ] FR-22: Add config message tests (SetHarness, SetModel, etc.)

## Acceptance Criteria
1. Given the Agents tab is selected, when the view loads, then pick_lists show available harnesses, tags, and agents
2. Given a model is typed in text_input, when Start Headless Swarm is clicked, then the model value is passed to the bridge
3. Given a task in Waves view, when Spawn is clicked, then it launches with current launch_config settings
4. Given the app starts, when initialization completes, then available_tags and available_agents are loaded
5. Given any configuration change, when the user modifies a field, then AppState.launch_config updates immediately

## Out of Scope
- Persisting launch_config to disk between sessions
- Agent TOML file editing from the GUI
- Multi-swarm management (only one active swarm)

## Dependencies
- Requires: scud-core Storage API for tag loading
- Requires: .scud/agents/ directory convention for agent discovery
