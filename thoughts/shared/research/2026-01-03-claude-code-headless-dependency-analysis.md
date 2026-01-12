---
date: 2026-01-04T03:02:12Z
researcher: Claude
git_commit: 8bd69099b3acfe2d6c10fed8ff59d9d409785c2e
branch: master
repository: scud
topic: "Claude Code Headless Mode for Dependency Analysis"
tags: [research, codebase, ai, dependencies, claude-code, headless]
status: complete
last_updated: 2026-01-03
last_updated_by: Claude
---

# Research: Claude Code Headless Mode for Dependency Analysis

**Date**: 2026-01-04T03:02:12Z
**Researcher**: Claude
**Git Commit**: 8bd69099b3acfe2d6c10fed8ff59d9d409785c2e
**Branch**: master
**Repository**: scud

## Research Question

How to add/modify dependency analysis commands to use Claude Code in headless mode instead of the current LLM client approach, and what architectural changes would be required?

## Summary

SCUD already has a `claude-cli` provider option that invokes Claude Code in headless mode via the `-p` flag. A new dependency analysis command using Claude Code headless mode could either:

1. **Extend the existing `claude-cli` provider** - Minimal changes, uses current infrastructure
2. **Create a standalone command** - Bypasses LLMClient entirely, calls `claude` directly with enhanced prompts and codebase access

The key architectural decision is whether to use Claude Code's built-in tools (Read, Grep, Glob) for codebase analysis rather than just sending text prompts. This would be a significant departure from the current "assistant" pattern where all context is serialized into the prompt.

## Detailed Findings

### Current Dependency Analysis Implementation

**Location**: `scud-cli/src/commands/ai/reanalyze_deps.rs`

The `reanalyze-deps` command:
1. Loads all phases/tags from storage
2. Builds a text context of all tasks with their current state
3. Sends a single prompt to the LLM requesting dependency suggestions
4. Parses JSON response into `DependencySuggestion` structs
5. Applies changes to task dependencies

**Key Data Structures**:
```rust
#[derive(Debug, Deserialize)]
struct DependencySuggestion {
    task_id: String,
    add_dependencies: Vec<String>,
    remove_dependencies: Vec<String>,
    reasoning: String,
}
```

**Current Prompt** (`llm/prompts.rs:183-230`):
- Receives task context as a formatted string
- Asks for JSON array of suggestions
- No access to actual codebase files

### Current LLM Client Architecture

**Location**: `scud-cli/src/llm/client.rs`

Supports 5 providers:
| Provider | API Type | Auth | Notes |
|----------|----------|------|-------|
| `anthropic` | Native Anthropic | `ANTHROPIC_API_KEY` | Direct HTTP |
| `openai` | OpenAI-compatible | `OPENAI_API_KEY` | Direct HTTP |
| `xai` | OpenAI-compatible | `XAI_API_KEY` | Default provider |
| `openrouter` | OpenAI-compatible | `OPENROUTER_API_KEY` | Multi-model router |
| `claude-cli` | Local CLI | None (uses local auth) | **Headless mode** |

**Claude CLI Integration** (`client.rs:299-357`):
```rust
async fn complete_claude_cli(&self, prompt: &str, model_override: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")                    // Print mode (headless)
        .arg("--output-format").arg("json")
        .arg("--model").arg(model)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Write prompt to stdin, parse JSON response
}
```

**Current Limitation**: Only uses `-p` flag with stdin prompt. Does not leverage:
- `--allowedTools` for codebase access
- `--append-system-prompt` for context
- Session management (`--continue`, `--resume`)
- Structured output with `--json-schema`

### Claude Code Headless Capabilities

**Invocation Methods**:

1. **Basic Headless** (current SCUD implementation):
   ```bash
   claude -p "prompt" --output-format json
   ```

2. **With Tool Access**:
   ```bash
   claude -p "Analyze dependencies in this project" \
     --allowedTools "Read,Grep,Glob" \
     --output-format json
   ```

3. **With Structured Output**:
   ```bash
   claude -p "Suggest dependency changes" \
     --json-schema '{"type":"array","items":{"type":"object",...}}' \
     --output-format json
   ```

4. **With System Prompt Enhancement**:
   ```bash
   claude -p "prompt" \
     --append-system-prompt "Project context: ..." \
     --output-format json
   ```

**Key Flags for Enhanced Analysis**:
| Flag | Purpose |
|------|---------|
| `--allowedTools "Read,Grep,Glob"` | Auto-approve codebase exploration tools |
| `--json-schema` | Enforce structured JSON output |
| `--append-system-prompt` | Add project context without replacing defaults |
| `--max-turns N` | Limit agentic exploration depth |
| `--permission-mode acceptEdits` | For future auto-apply features |

### Command Structure for Adding New Commands

**Registration Pattern** (`main.rs`):
1. Add enum variant to `Commands`:
```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands
    AnalyzeDepsV2 {
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        all_tags: bool,
    },
}
```

2. Add match arm in `main()`:
```rust
Commands::AnalyzeDepsV2 { tag, all_tags } => {
    commands::ai::analyze_deps_v2::run(cli.project, tag.as_deref(), all_tags).await
}
```

3. Create implementation in `commands/ai/analyze_deps_v2.rs`

4. Export in `commands/ai/mod.rs`:
```rust
pub mod analyze_deps_v2;
```

### Architectural Options for Claude Code Integration

**Option A: Enhance Existing `claude-cli` Provider**

Modify `LLMClient::complete_claude_cli()` to support additional flags:

```rust
pub async fn complete_claude_cli_with_tools(
    &self,
    prompt: &str,
    allowed_tools: &[&str],
    json_schema: Option<&str>,
) -> Result<String> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg("--output-format").arg("json");

    if !allowed_tools.is_empty() {
        cmd.arg("--allowedTools").arg(allowed_tools.join(","));
    }

    if let Some(schema) = json_schema {
        cmd.arg("--json-schema").arg(schema);
    }
    // ...
}
```

**Pros**: Reuses existing infrastructure, minimal changes
**Cons**: Still goes through LLMClient abstraction

**Option B: Standalone Claude Code Command**

Create a new command that directly invokes `claude` with full agentic capabilities:

```rust
// commands/ai/analyze_deps_v2.rs
pub async fn run(project_root: Option<PathBuf>, ...) -> Result<()> {
    let storage = Storage::new(project_root.clone());
    let task_context = build_task_context(&storage.load_tasks()?);

    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg("--allowedTools").arg("Read,Grep,Glob")
        .arg("--output-format").arg("json")
        .arg("--json-schema").arg(DEPENDENCY_SCHEMA)
        .arg("--append-system-prompt").arg(&task_context);

    // Write analysis prompt to stdin
    // Claude can now read actual code files to understand dependencies
}
```

**Pros**: Full Claude Code power, can read actual code
**Cons**: Bypasses LLMClient, provider-specific

**Option C: Hybrid Approach**

Add a `claude-cli-agentic` provider or flag that enables tool use:

```rust
// config.rs
pub struct LLMConfig {
    pub provider: String,
    pub model: String,
    pub max_tokens: u32,
    pub enable_tools: bool,  // New field
}
```

**Pros**: Clean separation, configurable behavior
**Cons**: More complex configuration

### Key Differences: Current vs Claude Code Headless

| Aspect | Current Approach | Claude Code Headless |
|--------|------------------|---------------------|
| Context | Serialized task text | Can read actual code files |
| Analysis Depth | Surface-level from text | Can trace imports, follow references |
| Tool Use | None | Read, Grep, Glob available |
| Iterations | Single prompt/response | Multi-turn agentic exploration |
| Output | Parse free-form JSON | Enforce schema with `--json-schema` |
| Code Understanding | Text description only | Actual AST/code analysis |

### Prompt Enhancement for Code-Aware Analysis

Current prompt sends task titles/descriptions. Enhanced prompt could:

```
You have access to Read, Grep, and Glob tools. Analyze the codebase to understand:

1. Which tasks produce code artifacts (files, modules, APIs)
2. Which tasks consume artifacts from other tasks
3. Import/export relationships in the codebase
4. Data flow dependencies (database schemas, types, interfaces)

Tasks to analyze:
{task_context}

Use the tools to:
- Read key files mentioned in task descriptions
- Search for cross-references between components
- Identify missing dependencies based on code imports

Return suggestions as JSON array...
```

### Existing Claude CLI Integration Points

The project already has Claude CLI support:

1. **Provider config** (`config.rs:34-50`):
   - `claude-cli` is a valid provider option
   - Uses local Claude Code authentication

2. **Client implementation** (`client.rs:299-357`):
   - Spawns `claude` process
   - Sends prompt via stdin
   - Parses JSON response

3. **Model options** (`config.rs:85-88`):
   ```rust
   "claude-cli" => vec!["sonnet", "opus", "haiku"]
   ```

## Code References

- `scud-cli/src/commands/ai/reanalyze_deps.rs:20-159` - Main reanalyze-deps implementation
- `scud-cli/src/llm/client.rs:299-357` - Claude CLI provider implementation
- `scud-cli/src/llm/prompts.rs:183-230` - Dependency analysis prompt
- `scud-cli/src/main.rs:237-252` - ReanalyzeDeps command registration
- `scud-cli/src/config.rs:67-90` - Provider and model configuration

## Architecture Documentation

### Current AI Command Flow

```
User invokes command
       ↓
LLMClient.new() loads config
       ↓
Build text prompt with context
       ↓
LLMClient.complete_json() → Provider API/CLI
       ↓
Parse JSON response
       ↓
Apply changes to storage
```

### Proposed Claude Code Headless Flow

```
User invokes command
       ↓
Build minimal task context
       ↓
Invoke claude -p with:
  - --allowedTools for codebase access
  - --json-schema for structured output
  - --append-system-prompt for task context
       ↓
Claude reads codebase files as needed
       ↓
Returns structured dependency suggestions
       ↓
Apply changes to storage
```

## Open Questions

1. **Schema Enforcement**: Should we use `--json-schema` to guarantee output format, or continue with flexible parsing?

2. **Tool Limits**: What tools should be auto-approved? Just `Read,Grep,Glob` or also `Bash` for running test commands?

3. **Iteration Depth**: Should we use `--max-turns` to limit how much Claude explores, or let it run until done?

4. **Fallback Strategy**: If Claude Code isn't installed, should it fall back to the current text-only approach?

5. **Provider Abstraction**: Should this be a new provider (`claude-cli-agentic`) or a flag on the existing `claude-cli` provider?

6. **Session Persistence**: Could session IDs be used to maintain context across multiple `reanalyze-deps` runs?

## Related Research

- `thoughts/shared/research/2025-12-11-scud-dependency-waves-subtask-analysis.md` - Dependency waves analysis
- `thoughts/shared/plans/2025-12-02-cross-tag-dependencies.md` - Cross-tag dependency design
- `thoughts/shared/plans/2025-12-12-guidance-context-for-ai-commands.md` - AI command guidance
