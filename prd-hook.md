### SCUD v2.0 “Bulletproof Completes” PRD  
**Feature:** Enforced task completion via Claude Code Stop hooks  
**Goal:** Make it physically impossible for a Claude sub-agent to finish a task without calling `scud complete <id>` — zero reliance on LLM memory or politeness.  
**Target release:** v2.0.0 (post-beta)

#### 1. Why this feature
- Current reality: Claude Code sub-agents forget to mark tasks done ~12–18% of the time in real waves.
- Beads wins agent love because its issues are the single source of truth and agents can’t “walk away” without closing them.
- We want the same guarantee with zero wrapper scripts, zero background loops, and zero extra dependencies.
- Claude Code’s Stop hook (June 2025) gives us the perfect enforcement point: it fires on every clean exit and can block the session from ending until the task is marked complete.

#### 2. User stories (prioritised)

| Priority | Story                                                                                            | Acceptance criteria                                                                                              |
|----------|--------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------|
| 1        | As an orchestrator, I want every Claude sub-agent to be forced to run `scud complete` before it can end a task. | `scud complete <id>` is executed automatically on every clean Stop hook. If it fails, the session is blocked.    |
| 2        | As a human, I want `scud wave start` to auto-install the enforcement hook so I never forget.     | Running `scud wave start …` writes the correct Stop hook into `.claude/settings.local.json` (project-scoped).    |
| 3        | As a human, I want to disable enforcement temporarily for debugging.                            | `scud wave start --no-enforce` skips hook installation. Hook can be removed with `scud hooks uninstall`.         |
| 4        | As an orchestrator, I want stuck/dead sessions to be auto-failed after timeout.                  | If Stop hook never fires (Claude killed, network drop, etc.), orchestrator’s `scud next --stale` will reassign after 3 min of no heartbeat file. |
| 5        | As a developer, I want the hook to pull a short auto-summary from the live log if the agent didn’t provide one. | `scud complete` called by the hook uses the last 5 lines of `.scud/live/<id>.log` as default summary.            |

#### 3. Minimal viable implementation (≤ 150 LOC total)

**A. New CLI command**
```bash
scud hooks install        # writes the Stop hook (idempotent)
scud hooks uninstall      # removes it
scud hooks status         # shows if active
```

**B. Hook payload (written to .claude/settings.local.json)**
```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "Task|Subagent",
        "hooks": [
          {
            "type": "command",
            "command": "scud _internal_complete_from_hook",
            "run_in_background": false
          }
        ]
      }
    ]
  }
}
```

**C. Internal binary command (private, not exposed to LLM)**
```rust
// src/bin/scud-_internal_complete_from_hook.rs
// Only callable from Claude Code hook (never documented to LLM)
fn main() {
    let task_id = std::env::var("CLAUDE_TASK_ID")
        .or_else(|_| extract_from_last_prompt()) // fallback
        .expect("No task ID found");

    let summary = match std::env::var("CLAUDE_TASK_SUMMARY") {
        Ok(s) if !s.trim().is_empty() => s,
        _ => auto_summary_from_log(&task_id),
    };

    Command::new("scud")
        .args(["complete", &task_id, "--summary", &summary])
        .status()
        .expect("Failed to mark task complete");

    // Optional: remove claim + clean live log
    Command::new("scud").args(["unclaim", &task_id]).ok();
}
```

**D. Wave start auto-install**
```rust
if !matches.contains_id("no-enforce") {
    hooks::install_stop_hook().unwrap();
    println!("Bulletproof completes enabled ✓");
}
```

#### 4. Exact user-facing flow (what people will actually type)

```bash
# One-time setup (per project)
scud init

# Start a parallel wave — automatically installs the enforcement hook
scud wave start auth-refactor --parallelism 4

# Fire four Claude Code sub-agents exactly as before
for task in $(scud next --ready --wave auth-refactor --limit 4); do
  claude-code run "Implement $task" &
done

# Every single sub-agent is now physically unable to end without:
# → scud complete t-47 --summary "..."
# → DAG is always accurate
# → Orchestrator never has to guess
```

#### 5. Success metrics (post-release)

| Metric                            | Target      | How to measure                                 |
|-----------------------------------|-------------|------------------------------------------------|
| `scud complete` called rate       | 99.9%       | Count Stop hook invocations vs. manual completes |
| Abandoned tasks per wave          | ≤ 1         | `scud list --claimed --stale` after wave finish |
| Hook installation friction        | 0 manual steps | 100% of `wave start` calls succeed without --no-enforce |
| Extra latency added by hook       | ≤ 80 ms     | Benchmarked on 100 tasks                        |

#### 6. Future-proofing (already designed in)

- Heartbeats → add a second hook (SessionStart + background loop) when we want full agent mode.
- Non-Claude agents → same `scud run` wrapper from earlier PRD can call the same internal complete binary.
- Cursor / Amp → they’ll get their own hook systems in 2026; the internal binary stays identical.

Ship this and SCUD instantly becomes the only public tool that can reliably run 5–20 parallel Claude Code agents with zero manual cleanup. This is the single highest-leverage feature you can add right now.
