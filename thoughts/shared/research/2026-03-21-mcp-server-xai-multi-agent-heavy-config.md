---
date: 2026-03-21T18:22:43Z
researcher: reuben
git_commit: 7c43e1b
branch: master
repository: pyrex41/scud
topic: "MCP server for scud CLI, xAI multi-agent as heavy branch, model configurability"
tags: [research, mcp, cowork, heavy, xai, multi-agent, configuration]
status: complete
last_updated: 2026-03-21
last_updated_by: reuben
---

# Research: MCP Server for SCUD, xAI Multi-Agent Heavy Branch, Model Configurability

**Date**: 2026-03-21T18:22:43Z
**Researcher**: reuben
**Git Commit**: 7c43e1b
**Branch**: master
**Repository**: pyrex41/scud

## Research Questions

1. How to expose scud as an MCP server for Anthropic Cowork users
2. Granular model control for scud heavy (cost: grok-4.20 is 10x grok-4.1)
3. Using xAI native multi-agent as one branch of scud heavy alongside individual rho agents
4. Easy runtime configuration for all of the above

## Summary

Three concrete work items emerge from this research:

**A) Build `scud-mcp` — an MCP server binary that wraps scud CLI commands as MCP tools.** Since scud is Go, the best approach is using `mcp-go` (github.com/mark3labs/mcp-go) or the official Go SDK (github.com/modelcontextprotocol/go-sdk) to build a native binary. The key design choice is tiered tool exposure (5-7 core tools, not all 30+) to avoid burning 20K+ context tokens. A critical Cowork limitation: stdio MCP payloads >~1KB are silently dropped in Claude Desktop — tool arguments must stay small.

**B) Add a `--mode` flag to `scud heavy` that supports running the xAI native multi-agent API as one "branch" alongside the rho individual agent ensemble**, then synthesizing both results. The native mode already exists (`--native` flag) but is either/or — it needs to be composable.

**C) Expand `--model` to support per-role overrides** so users can cheaply run agents on grok-4.1 while reserving grok-4.20 for synthesis/captain, all configurable via flags, env vars, or config.toml.

---

## Detailed Findings

### 1. MCP Server for Cowork Integration

#### What MCP Is

MCP (Model Context Protocol) is Anthropic's JSON-RPC 2.0 protocol for exposing tools to AI agents. A server exposes tools via `tools/list` and handles `tools/call` requests. Transport is typically stdio (server as subprocess) or HTTP.

#### Go MCP Libraries

Two options for scud (a Go project):

| Library | URL | Notes |
|---------|-----|-------|
| `mark3labs/mcp-go` | github.com/mark3labs/mcp-go | Most popular community Go SDK, battle-tested |
| `modelcontextprotocol/go-sdk` | github.com/modelcontextprotocol/go-sdk | Official, Google-maintained, newer |

A minimal Go MCP server wrapping scud:

```go
package main

import (
    "context"
    "os/exec"
    "github.com/mark3labs/mcp-go/mcp"
    "github.com/mark3labs/mcp-go/server"
)

func main() {
    s := server.NewMCPServer("scud", "1.0.0")
    s.AddTool(
        mcp.NewTool("scud_next",
            mcp.WithDescription("Find the next available task based on DAG dependencies"),
        ),
        func(ctx context.Context, req mcp.CallToolRequest) (*mcp.CallToolResult, error) {
            out, _ := exec.Command("scud", "next").CombinedOutput()
            return mcp.NewToolResultText(string(out)), nil
        },
    )
    server.ServeStdio(s)
}
```

#### Cowork Configuration

Cowork uses `claude_desktop_config.json`:
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "scud": {
      "command": "/path/to/scud-mcp",
      "args": [],
      "env": { "SCUD_TOOLS": "warmup,next,show,set-status,commit,stats,heavy" }
    }
  }
}
```

#### Critical Cowork Limitation: 1KB Payload Bug

**GitHub issue anthropics/claude-code#36319**: Claude Desktop (Chat/Cowork mode) silently drops MCP stdio tool calls when the JSON-RPC payload exceeds ~800-1,100 bytes. The request never reaches the server — it's dropped client-side.

Implications for scud-mcp:
- Tool arguments must stay small (task IDs, status strings, short messages)
- Long prompts (e.g., `scud heavy` queries) should be written to a temp file and passed as a path
- Or use HTTP transport instead of stdio to avoid this

This does NOT affect Claude Code CLI or VSCode — only Desktop Chat/Cowork.

#### Tool Exposure Strategy

Each MCP tool definition costs 550-1,400 context tokens. With 30+ scud commands, exposing all would burn 20K+ tokens. Strategies:

| Strategy | Tokens | Trade-off |
|----------|--------|-----------|
| Single dispatch tool (`scud(args)`) | ~100 | Flexible but no schema safety |
| 5-7 grouped core tools | ~2,000 | Best balance |
| 15 tiered tools | ~8,000 | Comprehensive but heavy |
| All 30+ commands | ~21,000 | Context-destroying |

**Recommended**: Env-var controlled tiering (like task-master-ai does):
- `SCUD_TOOLS=core` → warmup, next, show, set-status, commit, stats (6 tools, ~3K tokens)
- `SCUD_TOOLS=full` → all commands
- `SCUD_TOOLS=next,show,heavy` → custom selection

#### Alternative: Zero-Code Auto-Wrapper

`any-cli-mcp-server` (npm) parses `--help` output and auto-generates MCP tools:
```json
{
  "mcpServers": {
    "scud": {
      "command": "npx",
      "args": ["-y", "any-cli-mcp-server", "scud"]
    }
  }
}
```
Quick to try but quality depends on help text structure. Good for prototyping.

---

### 2. Current Model Configuration in scud heavy

#### Current State (`internal/config/config.go`, `internal/heavy/ensemble.go`)

The heavy ensemble currently uses a **single model for all agents**:

- `--model` flag → `cfg.Heavy.Model` → `cfg.Rho.SmartModel` → hardcoded `"grok-4.20-reasoning"`
- Every agent (routing, execution, synthesis, debate) uses the same model
- The `--native` flag switches to xAI multi-agent API entirely (either/or, not composable)

Config hierarchy (config.toml):
```toml
[heavy]
model = ""              # if empty, falls back to rho.smart_model
concurrency = 4
timeout_secs = 300

[rho]
smart_model = "grok-4.20-beta-0309-reasoning"
fast_model = "grok-code-fast-1"
```

Env overrides: `SCUD_HEAVY_MODEL`, `SCUD_SMART_MODEL`, `SCUD_HEAVY_CONCURRENCY`

#### Cost Problem

All grok-4.20 variants: $2.00/$6.00 per 1M tokens (input/output)
grok-4.1-fast: $0.20/$0.50 per 1M tokens — **10x cheaper**

With 4 agents + captain routing + synthesis, a single heavy query at grok-4.20 rates costs ~15-80x what it would on grok-4.1-fast, depending on output length.

#### What's Needed: Per-Role Model Overrides

Proposed config structure:
```toml
[heavy]
concurrency = 4
timeout_secs = 300

[heavy.models]
routing = "grok-4.1-fast"          # Captain agent selection (cheap, structured)
agents = "grok-4.1-fast"           # Individual specialist agents
synthesis = "grok-4.20-reasoning"  # Captain synthesis (needs quality)
debate = "grok-4.1-fast"           # Critique rounds (if enabled)
```

CLI override: `--model-agents grok-4.1-fast --model-synthesis grok-4.20-reasoning`

Env override: `SCUD_HEAVY_MODEL_AGENTS`, `SCUD_HEAVY_MODEL_SYNTHESIS`

This would let the typical heavy query use grok-4.1-fast for the bulk of work (4 agents reading files, routing) and only use grok-4.20 for the final synthesis — cutting cost by ~5-8x.

---

### 3. xAI Native Multi-Agent as a Heavy Branch

#### Current Native Mode (`--native` flag)

`RunNative()` in `ensemble.go:57-121`:
- Calls the xAI Responses API directly (`https://api.x.ai/v1/responses`)
- Model: `grok-4.20-multi-agent-beta-0309`
- Effort: `"low"` (4 agents) or `"high"/"xhigh"` (16 agents)
- Server-side tools: `web_search`, `x_search`, `code_execution`
- Returns a single synthesized response — no access to individual agent outputs

This is currently **either/or** with the rho ensemble — `--native` bypasses everything.

#### xAI Multi-Agent Architecture

The xAI API runs 4 named agents internally that mirror scud's own:

| xAI Agent | Role | scud Equivalent |
|-----------|------|-----------------|
| Grok (Captain) | Decomposition, routing, synthesis | Captain |
| Harper | Research, fact-checking, web/X search | Harper |
| Benjamin | Logic, math, code validation | Benjamin |
| Lucas | Contrarian/creative analysis | Lucas |

All agents share base weights and KV cache — overhead is 1.5-2.5x a single run, not 4x.

**Pricing**: Same $2/$6 per 1M tokens as other grok-4.20 models, but all sub-agent tokens are billed. Web search: $5 per 1K queries.

**Limitation**: No custom client-side tools in multi-agent mode. The Responses API only supports server-side tools (web_search, x_search, code_execution). So the native multi-agent cannot read local files — it's complementary to the rho ensemble which CAN read files.

#### Proposed: Hybrid Mode

Run both the rho ensemble AND xAI native multi-agent in parallel, then synthesize:

```
scud heavy --mode hybrid "query"
```

Execution flow:
1. Captain routes query to specialists (same as today)
2. In parallel:
   a. rho agents execute with file tools (local codebase analysis)
   b. xAI native multi-agent executes with web_search/x_search (external research)
3. Captain synthesizes both branches

This gives the best of both worlds:
- rho agents: file access, local codebase knowledge
- xAI native: web search, X search, code execution sandbox

Implementation:
```go
// In executeAgents, add native as a special "agent"
if opts.Mode == "hybrid" || opts.Mode == "both" {
    g.Go(func() error {
        nativeResult, err := RunNative(ctx, cfg, RunOpts{
            Query:        query,
            NativeEffort: "low",
            NativeTools:  []string{"web_search"},
        })
        // Add to outputs as a special AgentOutput
        mu.Lock()
        outputs = append(outputs, AgentOutput{
            Name:   "xAI-MultiAgent",
            Domain: "Web Research & Cross-Reference",
            Output: nativeResult.Synthesis,
        })
        mu.Unlock()
        return nil
    })
}
```

#### Mode Flag Design

```
--mode ensemble   (default) rho agents only, file tools
--mode native     xAI multi-agent only, web tools
--mode hybrid     both in parallel, captain synthesizes
```

Config:
```toml
[heavy]
mode = "ensemble"  # default mode
```

Env: `SCUD_HEAVY_MODE=hybrid`

---

### 4. Cowork-Specific Considerations

#### Cowork Architecture

Cowork runs inside a **sandboxed Linux VM** on the user's machine (Apple Virtualization Framework on Mac). It:
- Can decompose tasks into parallel sub-agents
- Outputs files (.docx, .pptx, .xlsx, .pdf) into a shared folder
- Shares MCP configuration with Chat and Code tabs
- Memory persists within a project but NOT across standalone sessions

#### MCP Integration Points for scud

For Cowork users who want scud as a tool:
1. Install scud binary (via install.sh)
2. Install scud-mcp binary (or use `any-cli-mcp-server`)
3. Add to `claude_desktop_config.json`
4. Use `SCUD_TOOLS=core` for minimal context overhead

The agent using scud via MCP would call tools like:
- `scud_next()` → returns next available task
- `scud_show(id: "1.2")` → returns task details
- `scud_set_status(id: "1.2", status: "done")` → marks complete
- `scud_heavy(query: "...", model_agents: "grok-4.1-fast")` → runs heavy with cost control

For the heavy tool specifically, the model override is critical — the Cowork user needs to control cost without editing config files.

---

## Code References

- `internal/config/config.go` — Config struct, model resolution, env overrides
- `internal/heavy/ensemble.go` — Run(), RunNative(), executeAgents(), resolveModel()
- `internal/heavy/agents.go` — 16-agent registry, core vs specialist split
- `internal/heavy/prompts.go` — Routing, synthesis, critique prompt builders
- `internal/cmd/heavy.go` — CLI flags, RunOpts construction
- `internal/llm/provider.go` — xaiResponsesProvider.CompleteMultiAgent()
- `internal/llm/types.go` — ResponsesRequest, ResponsesReasoning (effort parameter)
- `internal/rho/rho.go` — Run(), RunStreaming(), AdaptiveTimeout

## Architecture: Current Heavy Execution Flow

```
User: scud heavy "query" --verbose

1. resolveModel() → single model for everything
2. routeAgents() → Captain selects specialists via rho-cli (60s timeout)
3. mergeAgents() → core 4 + selected specialists
4. executeAgents() → parallel rho-cli with streaming + adaptive timeout
5. synthesize() → Captain merges outputs (120s timeout)
6. [optional] debate rounds → critique + resynthesize
7. PrintResult() → stdout
```

## Architecture: Proposed Heavy with Per-Role Models + Hybrid

```
User: scud heavy "query" --mode hybrid --model-agents grok-4.1-fast --model-synthesis grok-4.20-reasoning

1. resolveModels() → routing=fast, agents=fast, synthesis=smart, native=multi-agent
2. routeAgents(routing_model) → Captain selects specialists
3. mergeAgents()
4. In parallel:
   a. executeAgents(agent_model) → rho-cli agents with file tools
   b. RunNative(effort=low, tools=[web_search]) → xAI multi-agent
5. synthesize(synthesis_model) → Captain merges ALL outputs (rho + native)
6. PrintResult()
```

## Related Research

- `thoughts/shared/research/2026-03-12-rho-vs-axe-as-scud-backend.md`
- `thoughts/shared/research/2026-03-11-scud-go-clone-spec-full-analysis.md`

## External References

- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [mark3labs/mcp-go](https://github.com/mark3labs/mcp-go) — Go MCP SDK
- [Official Go MCP SDK](https://github.com/modelcontextprotocol/go-sdk)
- [any-cli-mcp-server](https://github.com/eirikb/any-cli-mcp-server) — zero-code CLI wrapper
- [xAI Multi-Agent Docs](https://docs.x.ai/developers/model-capabilities/text/multi-agent)
- [xAI Models & Pricing](https://docs.x.ai/developers/models)
- [Cowork intro](https://claude.com/blog/cowork-research-preview)
- [Cowork architecture](https://claudecn.com/en/blog/claude-cowork-architecture/)
- [Cowork 1KB stdio bug](https://github.com/anthropics/claude-code/issues/36319)
- [Task Master MCP tiered tools](https://docs.task-master.dev/capabilities/mcp)
- [Context window token costs](https://www.apideck.com/blog/mcp-server-eating-context-window-cli-alternative)

## Open Questions

1. Is the Cowork 1KB stdio payload bug fixed in current Claude Desktop builds?
2. Does 16-agent xAI multi-agent (`effort: "high"`) actually work via the API, or is it consumer-only (SuperGrok)?
3. Should the hybrid mode synthesize rho+native outputs together, or should the Captain see them as separate "branches" with different credibility weights?
4. For the MCP server: should it be a separate binary (`scud-mcp`) or a subcommand (`scud mcp-server`)?
5. Would OpenRouter be a better default for multi-provider routing than direct API calls?
