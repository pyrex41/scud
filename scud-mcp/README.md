# SCUD MCP Server

Model Context Protocol (MCP) server for [SCUD](https://github.com/pyrex41/scud) task management. Enables AI assistants like Claude Desktop to interact with SCUD's task management and AI-powered workflow features.

## Overview

**SCUD MCP** wraps the SCUD CLI and exposes it through the Model Context Protocol, allowing any MCP-compatible client to:
- Parse PRDs into tasks using AI
- Manage tags and task lifecycles
- Track workflow state and progress
- Coordinate parallel development across teams
- Analyze complexity and expand tasks automatically

Perfect for teams using Claude Desktop, Cline, or other MCP clients who want AI-powered task management integrated into their development workflow.

## Features

### 🛠️ **20 MCP Tools** (Actions)

#### Core Commands
- `scud_init` - Initialize SCUD in current directory
- `scud_list` - List tasks (optionally filter by status)
- `scud_next` - Find next available task
- `scud_stats` - Show statistics
- `scud_show` - Show task details
- `scud_set_status` - Update task status

#### Tag Management
- `scud_tags` - List all tags
- `scud_use_tag` - Set active tag

#### AI-Powered (Requires XAI_API_KEY)
- `scud_parse_prd` - Parse PRD markdown into tasks
- `scud_analyze_complexity` - Analyze task complexity (Fibonacci scale)
- `scud_expand` - Break down complex tasks into subtasks

#### Parallel Development
- `scud_create_group` - Create tag group for parallel work
- `scud_list_groups` - List all tag groups
- `scud_group_status` - Show group progress
- `scud_assign` - Assign task to developer
- `scud_claim` - Claim task for yourself
- `scud_release` - Release claimed task
- `scud_whois` - Show task assignments

### 📊 **3 MCP Resources** (Read-only Data)
- `scud://workflow/state` - Current workflow state (JSON)
- `scud://tasks/list` - All tasks in active tag (JSON)
- `scud://stats/current` - Tag statistics (text)

## Installation

### Prerequisites

1. **Node.js 18+** - Required for MCP server
2. **SCUD CLI** - The Rust CLI must be installed and in PATH

```bash
# Install SCUD CLI (if not already installed)
npm install -g scud

# Verify installation
scud --version
```

### Install SCUD MCP Server

```bash
npm install -g scud-mcp
```

### Configure in Claude Desktop

Edit your Claude Desktop config file:

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
**Linux:** `~/.config/Claude/claude_desktop_config.json`

Add the SCUD MCP server:

```json
{
  "mcpServers": {
    "scud": {
      "command": "scud-mcp",
      "env": {
        "XAI_API_KEY": "xai-your-api-key-here"
      }
    }
  }
}
```

**Note:** The `XAI_API_KEY` is only required for AI-powered tools (`scud_parse_prd`, `scud_analyze_complexity`, `scud_expand`). Core task management works without it. Alternative providers: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`.

### Restart Claude Desktop

Restart Claude Desktop to load the MCP server. You should see the SCUD tools available in your conversations.

## Usage

### Example 1: Initialize Project and Parse PRD

**You (in Claude Desktop):**
> Initialize SCUD and parse my PRD file at docs/features/auth.md with tag auth

**Claude:**
```
[calls scud_init]
✓ Initialized SCUD successfully

[calls scud_parse_prd with file="docs/features/auth.md", tag="auth"]
✓ Parsed PRD into 12 tasks
✓ Active tag set to: auth
```

### Example 2: Development Workflow

**You:**
> What's my next task?

**Claude:**
```
[calls scud_next]
Next available task:

TASK-1: Set up authentication database schema
Complexity: 5
Priority: high
Status: pending
```

**You:**
> Mark TASK-1 as in-progress and show me the details

**Claude:**
```
[calls scud_set_status with task_id="TASK-1", status="in-progress"]
✓ Task TASK-1 updated to in-progress

[calls scud_show with task_id="TASK-1"]

TASK-1: Set up authentication database schema
Description: Create User and Session tables with proper indexes...
Complexity: 5 (Medium)
Priority: high
Status: in-progress
Dependencies: None
```

### Example 3: AI-Powered Complexity Analysis

**You:**
> Analyze the complexity of all tasks

**Claude:**
```
[calls scud_analyze_complexity]

Analyzing 12 tasks...

TASK-3: Implement OAuth2 flow
  Complexity: 21 → Recommended: EXPAND
  Reasoning: Involves multiple components (auth provider integration, token management,
  refresh logic, error handling). Should be broken down.

TASK-7: Add login form UI
  Complexity: 3
  Reasoning: Simple form with email/password fields. Well-defined scope.

[... more analysis ...]
```

**You:**
> Expand TASK-3

**Claude:**
```
[calls scud_expand with task_id="TASK-3"]

Expanded TASK-3 into 5 subtasks:

TASK-3-1: Research OAuth2 providers (complexity: 2)
TASK-3-2: Implement authorization endpoint (complexity: 5)
TASK-3-3: Implement token endpoint (complexity: 5)
TASK-3-4: Add token refresh logic (complexity: 3)
TASK-3-5: Add OAuth2 error handling (complexity: 3)
```

### Example 4: Parallel Development

**You:**
> Create a sprint group with auth, api, and frontend tags

**Claude:**
```
[calls scud_create_group with name="sprint-1", tags="auth,api,frontend"]
✓ Created group: sprint-1

[calls scud_group_status with group_id="sprint-1"]

Sprint 1 Status:
  auth: 5/12 tasks done (42%)
  api: 2/8 tasks done (25%)
  frontend: 0/15 tasks done (0%)

Overall: 7/35 tasks completed (20%)
```

### Example 5: Using Resources

**You:**
> Read the current workflow state

**Claude:**
```
[reads scud://workflow/state resource]

{
  "active_tag": "auth",
  "current_phase": "development",
  "phases": {
    "planning": { "status": "completed", "completed_at": "2025-11-15T10:00:00Z" },
    "development": { "status": "in-progress", "started_at": "2025-11-15T14:00:00Z" }
  },
  "completed_epics": []
}
```

## Tool Reference

### Core Tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `scud_init` | - | Initialize SCUD in current directory |
| `scud_list` | `status?` | List tasks (filter by status) |
| `scud_next` | - | Find next available task |
| `scud_stats` | - | Show statistics |
| `scud_show` | `task_id` | Show task details |
| `scud_set_status` | `task_id`, `status` | Update task status |

### Tag Management

| Tool | Parameters | Description |
|------|-----------|-------------|
| `scud_tags` | - | List all tags |
| `scud_use_tag` | `tag` | Set active tag |

### AI Tools (Require XAI_API_KEY)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `scud_parse_prd` | `file`, `tag` | Parse PRD into tasks |
| `scud_analyze_complexity` | `task?` | Analyze task complexity |
| `scud_expand` | `task_id?`, `all?` | Expand complex tasks |

### Parallel Development

| Tool | Parameters | Description |
|------|-----------|-------------|
| `scud_create_group` | `name`, `tags`, `description?` | Create tag group |
| `scud_list_groups` | - | List all tag groups |
| `scud_group_status` | `group_id` | Show group status |
| `scud_assign` | `task_id`, `assignee` | Assign task to developer |
| `scud_claim` | `task_id`, `name` | Claim task for yourself |
| `scud_release` | `task_id`, `force?` | Release claimed task |
| `scud_whois` | - | Show task assignments |

### Resource URIs

| Resource | Description | Format |
|----------|-------------|--------|
| `scud://workflow/state` | Current workflow state | JSON |
| `scud://tasks/list` | All tasks in active tag | JSON |
| `scud://stats/current` | Tag statistics | Text |

## Task Status Values

When using `scud_set_status`, valid status values are:
- `pending` - Not started
- `in-progress` - Currently being worked on
- `done` - Completed
- `review` - Ready for review
- `blocked` - Blocked by dependencies or issues
- `deferred` - Postponed to later
- `cancelled` - No longer needed

## Complexity Scale

SCUD uses Fibonacci complexity scores:
- `1` - Trivial (5-15 minutes)
- `2` - Simple (15-30 minutes)
- `3` - Moderate (30-60 minutes)
- `5` - Medium (1-3 hours)
- `8` - Complex (3-8 hours)
- `13` - Large (8+ hours, recommend expanding)
- `21` - Too large (must expand into subtasks)

## Troubleshooting

### "SCUD CLI not found in PATH"

The MCP server couldn't find the `scud` command. Install it:

```bash
npm install -g scud
```

Verify:

```bash
which scud  # macOS/Linux
where scud  # Windows
```

### "API key environment variable not set"

AI tools require an API key. Add it to your Claude Desktop config:

```json
{
  "mcpServers": {
    "scud": {
      "command": "scud-mcp",
      "env": {
        "XAI_API_KEY": "xai-your-key"
      }
    }
  }
}
```

Alternative keys: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `OPENROUTER_API_KEY`.

### "No active tag set"

Initialize a project and set an active tag:

```
1. Run scud_init
2. Run scud_parse_prd to create tasks with a tag
3. Run scud_use_tag to set the active tag
```

### Tools not showing in Claude Desktop

1. Check that claude_desktop_config.json is valid JSON
2. Restart Claude Desktop completely
3. Check logs: Help → View Logs in Claude Desktop
4. Verify scud-mcp is installed: `which scud-mcp`

## Development

### Build from Source

```bash
git clone https://github.com/pyrex41/scud
cd scud/scud-mcp
npm install
npm run build
npm link  # Test locally
```

### Project Structure

```
scud-mcp/
├── src/
│   ├── index.ts           # MCP server entry point
│   ├── types.ts           # TypeScript types
│   ├── tools/             # Tool implementations
│   │   ├── core.ts        # Core commands
│   │   ├── tags.ts        # Tag management
│   │   ├── task.ts        # Task operations
│   │   ├── ai.ts          # AI-powered tools
│   │   └── parallel.ts    # Parallel development
│   ├── resources/         # Resource implementations
│   │   ├── workflow.ts    # Workflow state
│   │   ├── tasks.ts       # Task lists
│   │   └── stats.ts       # Statistics
│   └── utils/
│       └── exec.ts        # CLI execution wrapper
├── package.json
├── tsconfig.json
└── README.md (this file)
```

## Performance

The SCUD MCP server is a lightweight wrapper around the fast Rust CLI:
- **Startup:** <100ms
- **Tool execution:** Inherits SCUD CLI performance (42ms average)
- **Memory:** <10MB for server process
- **Concurrent requests:** Supported (file locking in CLI prevents corruption)

## Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see LICENSE file for details

## Related Projects

- [SCUD CLI](https://github.com/pyrex41/scud/tree/master/scud-cli) - The Rust CLI this server wraps
- [Model Context Protocol](https://modelcontextprotocol.io/) - MCP specification
- [Claude Desktop](https://claude.ai/download) - MCP-compatible AI assistant

## Support

- Issues: https://github.com/pyrex41/scud/issues
- Documentation: https://github.com/pyrex41/scud/tree/master/docs
- Quick Reference: https://github.com/pyrex41/scud/blob/master/docs/reference/QUICK_REFERENCE.md

---

**Built with ❤️ for AI-powered development workflows**
