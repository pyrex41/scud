---
date: 2025-12-03T17:45:00-08:00
researcher: Claude
git_commit: 6a84e6b4b7dc2715690df9024db48afc5ce2303c
branch: master
repository: scud
topic: "scud init default model and defunct agents prompt"
tags: [research, codebase, init, configuration, agents]
status: complete
last_updated: 2025-12-03
last_updated_by: Claude
---

# Research: scud init Default Model and Defunct Agents Prompt

**Date**: 2025-12-03T17:45:00-08:00
**Researcher**: Claude
**Git Commit**: 6a84e6b4b7dc2715690df9024db48afc5ce2303c
**Branch**: master
**Repository**: scud

## Research Question

Why does `scud init` still not default to `grok-code-fast-1`, and why does it still ask about defunct agents?

## Summary

Two issues were identified:

1. **Model Default Discrepancy**: The NPM install script (`bin/install.js`) defaults xAI to `grok-4-1-fast-reasoning`, not `grok-code-fast-1`. The Rust CLI correctly defaults to `grok-code-fast-1`.

2. **Defunct Agents Prompt**: The NPM install script still prompts about installing workflow agents (pm, sm, architect, dev, retrospective, status), but these agent files do not exist in the repository.

## Detailed Findings

### Issue 1: Model Default Not `grok-code-fast-1`

#### NPM Install Script (`bin/install.js`)

At lines 169-176, the xAI provider is configured with `grok-4-1-fast-reasoning` as the default:

```javascript
const providers = [
  {
    name: 'xAI (Grok)',
    id: 'xai',
    model: 'grok-4-1-fast-reasoning',  // <-- WRONG DEFAULT
    env: 'XAI_API_KEY',
    models: ['grok-4-1-fast-reasoning', 'grok-4-1-fast', 'grok-3-fast', 'grok-code-fast-1']
  },
  // ...
];
```

**Problems:**
- Line 173: Default model is `grok-4-1-fast-reasoning` instead of `grok-code-fast-1`
- Line 175: `grok-code-fast-1` is listed LAST in the models array (should be first)

#### Rust CLI (`scud-cli/src/config.rs`)

The Rust implementation correctly defaults to `grok-code-fast-1`:

- Line 24: `Config::default()` sets model to `"grok-code-fast-1"`
- Line 80: `default_model_for_provider("xai")` returns `"grok-code-fast-1"`
- Lines 92-97: `suggested_models_for_provider("xai")` lists `"grok-code-fast-1"` first

**Discrepancy**: When users run `node bin/install.js` they get `grok-4-1-fast-reasoning`, but when they run `scud init` they get `grok-code-fast-1`.

### Issue 2: Defunct Agents Prompt

#### Prompt Location (`bin/install.js:296-322`)

The script still prompts users about installing workflow agents:

```javascript
log('SCUD includes workflow agents for Claude Code:', 'blue');
log('  • /scud-pm          - Product Manager (PRD creation)', 'reset');
log('  • /scud-sm          - Scrum Master (task breakdown)', 'reset');
log('  • /scud-architect   - Technical design', 'reset');
log('  • /scud-dev         - Task implementation', 'reset');
log('  • /scud-retrospective - Post-phase analysis', 'reset');
log('  • /status           - Workflow status', 'reset');

installAgents = await askYesNo('Install SCUD workflow agents?', true);
```

#### Agent Files Referenced (`bin/install.js:142`)

```javascript
const scudAgents = ['pm.md', 'sm.md', 'architect.md', 'dev.md', 'retrospective.md', 'status.md'];
```

#### Agent Files Status

**These files DO NOT EXIST in the repository.** The `.claude/commands/scud/` directory only contains task management commands:
- task-claim.md
- task-doctor.md
- task-list.md
- task-next.md
- task-show.md
- task-stats.md
- task-status.md
- task-tags.md
- task-waves.md
- task-whois.md

#### Planned Deprecation

Per `thoughts/shared/plans/2025-12-01-scud-v2-beads-inspired-refactor.md`:
- Line 35: "**No agent roles** - Slash commands removed; any agent can work on any task"
- Lines 845-847: `/scud:pm`, `/scud:sm`, etc. commands listed as removed

## Code References

### NPM Install Script
- `bin/install.js:169-176` - Provider definitions with wrong xAI default
- `bin/install.js:142` - Agent files list (files don't exist)
- `bin/install.js:296-322` - Agent installation prompt
- `bin/install.js:334-369` - Agent copy logic

### Rust CLI (correct implementation)
- `scud-cli/src/config.rs:19-28` - Config::default() with correct model
- `scud-cli/src/config.rs:78-87` - default_model_for_provider()
- `scud-cli/src/config.rs:90-121` - suggested_models_for_provider()

## Required Fixes

### Fix 1: Update Model Default in `bin/install.js`

Lines 169-176 should be:
```javascript
{
  name: 'xAI (Grok)',
  id: 'xai',
  model: 'grok-code-fast-1',  // Changed from grok-4-1-fast-reasoning
  env: 'XAI_API_KEY',
  models: ['grok-code-fast-1', 'grok-4-1-fast-reasoning', 'grok-4-1-fast', 'grok-3-fast']  // Reordered
}
```

### Fix 2: Remove Defunct Agents Prompt from `bin/install.js`

Remove or skip:
- Lines 296-322: Agent installation prompt
- Lines 334-369: Agent copy logic
- Line 142: Agent files array

The success message at lines 358-362 already correctly shows task commands instead of agent commands, suggesting partial cleanup was done but not completed.

## Architecture Documentation

The initialization system has two entry points:
1. **NPM**: `bin/install.js` - JavaScript interactive installer
2. **Rust CLI**: `scud init` command via `scud-cli/src/commands/init.rs`

Both should produce identical configurations but currently diverge on default model selection.

## Related Research

None found in `thoughts/shared/research/`.

## Open Questions

1. Should `bin/install.js` be deprecated in favor of `scud init`?
2. Should the agent infrastructure in `scud-cli/src/commands/config.rs` also be removed?
