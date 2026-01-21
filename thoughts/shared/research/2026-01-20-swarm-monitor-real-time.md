### Monitor Real-Time Updates Enhancement

**Issue**: Bridge creates proxy after swarm completion, but monitor needs real-time updates during execution.

**Current Flow**:
1. Swarm spawns agents → Tmux windows created
2. Swarm completes → Bridge creates proxy session in `.scud/spawn/`
3. Monitor finds proxy → Shows agents

**Enhancement**: Create proxy immediately after spawning, update in real-time.

#### Code Changes

1. **Move Bridge Call Earlier** (swarm/mod.rs):
   ```rust
   // After each round spawn (execute_round), call bridge immediately
   execute_round(...)?;
   create_and_save_spawn_proxy(...)?;  // Add here
   ```

2. **Real-Time Updates**: Bridge updates existing proxy with new agents as rounds complete.

3. **Status Sync**: Monitor polls SCUD task status + tmux windows every 2-3 seconds.

#### Expected Result
- Monitor shows agents as soon as they're spawned
- Status updates in real-time (Starting → Running → Completed)
- No "Waiting for round completion" issue

#### Implementation Plan
- Move `create_and_save_spawn_proxy` call after `execute_round`
- Modify bridge to append new agents to existing proxy session
- Test with `scud swarm --limit 2` + `scud spawn monitor`

**Benefits**: Real-time visibility during swarm execution.