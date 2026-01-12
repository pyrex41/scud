---
date: 2025-12-12T16:01:06Z
researcher: reuben
git_commit: 9d5251d97ad9627b57192f337c259a1aed378179
branch: master
repository: scud
topic: "NPM Wrapper Command Interception and Rust CLI Consolidation"
tags: [research, codebase, npm, rust-cli, command-routing, view-command]
status: complete
last_updated: 2025-12-12
last_updated_by: reuben
---

# Research: NPM Wrapper Command Interception and Rust CLI Consolidation

**Date**: 2025-12-12T16:01:06Z
**Researcher**: reuben
**Git Commit**: 9d5251d97ad9627b57192f337c259a1aed378179
**Branch**: master
**Repository**: scud

## Research Question

The npm scud wrapper handles/intercepts commands: `scud init`, `scud view`, and `scud install`. Goal is to:
1. Have these commands handled by the Rust CLI instead
2. Make npm install just use the binary directly (no Node.js wrapper layer)
3. Make `scud view` launch the static webpage view (not be an alias for `scud list`)

## Summary

The SCUD project has a hybrid architecture where the npm package provides a Node.js wrapper (`bin/scud.js`) that intercepts certain commands and delegates others to a Rust binary. Currently:

- **`scud init`**: Handled in Node.js, delegates to `bin/install.js`
- **`scud view`**: Fully handled in Node.js (1600+ lines of code) - generates HTML and opens browser
- **`scud install`**: Handled in Node.js, copies slash commands to project directories

The Rust CLI already has an `init` command, and `view` is aliased to `list` (terminal output, not browser). To achieve the consolidation goals, the static webpage view would need to be ported to Rust, and the npm package restructured to be a pure binary wrapper.

## Detailed Findings

### Current Command Interception Architecture

#### Entry Point: `bin/scud.js`

**Command Parsing (lines 13-14)**:
```javascript
const command = process.argv[2];
const args = process.argv.slice(3);
```

**Command Classification (lines 17-25)**:
```javascript
const taskCommands = ['tags', 'use-tag', 'list', 'show', 'set-status', 'next', 'stats', ...];
const aiCommands = ['parse', 'parse-prd', 'analyze-complexity', 'expand', 'research'];
const versionCommands = ['--version', '-V'];
const rustCommands = [...taskCommands, ...aiCommands, ...versionCommands];
```

**JavaScript-Only Commands** (handled before Rust delegation):
- `view` - intercepted at line 154-160 (early return)
- `init` - switch case at line 193-194
- `install` - switch case at line 196-197
- `status` - switch case at line 199-200
- `validate` - switch case at line 202-203
- `help` / no command - default case

### Command: `scud init`

#### Node.js Implementation (`bin/scud.js:96-103`)
```javascript
function init() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  const result = spawnSync('node', [installScript, 'init'], { stdio: 'inherit' });
  ...
}
```

Delegates to `bin/install.js` which:
- Prompts for AI provider selection (lines 165-239)
- Creates `.scud/` directory structure (lines 241-290)
- Creates `config.toml` with provider/model (lines 268-279)
- Copies slash commands (lines 293-320)
- Updates `CLAUDE.md` (lines 323-388)

#### Rust Implementation (`scud-cli/src/commands/init.rs`)
The Rust CLI already has `init`:
- Defined at `main.rs:69-74`
- Routes to `commands::init::run()`
- Implementation at `commands/init.rs:10-108`
- Handles provider selection and storage initialization
- **Missing**: Slash command copying and `CLAUDE.md` updating

### Command: `scud view`

#### Node.js Implementation (`bin/scud.js:154-160, 218-1820`)

**Interception (lines 154-160)**:
```javascript
if (command === 'view') {
  runView().catch(error => { ... });
  return;  // Early return prevents Rust delegation
}
```

**`runView()` function (lines 218-266)**:
1. Validates `.scud` directory exists
2. Reads `tasks.scg` or `tasks.json`
3. Parses SCG format via `parseScgFile()` (lines 382-644)
4. Computes execution waves via `computeWaves()` (lines 273-376)
5. Generates HTML via `generateViewerHtml()` (lines 669-740)
6. Includes CSS via `getViewerStyles()` (lines 745-1202)
7. Includes JS via `getViewerScript()` (lines 1207-1809)
8. Writes to temp file and opens browser using `open` package

**Total implementation**: ~1600 lines of JavaScript

#### Rust Implementation (`scud-cli/src/main.rs:82-100`)

`view` is currently an **alias for `list`**:
```rust
#[command(alias = "view")]
List {
    #[arg(short, long)]
    status: Option<String>,
    ...
}
```

This outputs task list to terminal, **not** a browser view.

### Command: `scud install`

#### Node.js Implementation (`bin/scud.js:105-112`)
```javascript
function install() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  const result = spawnSync('node', [installScript, ...args], { stdio: 'inherit' });
  ...
}
```

Delegates to `bin/install.js` which copies slash commands from the npm package to the project's `.claude/commands/scud/` and `.opencode/command/scud/` directories.

#### Rust Implementation
**Does not exist.** There is no `install` subcommand in the Rust CLI. The closest is `config agents add` which handles agent installation but not the initial command copying.

### NPM Package Structure

#### Binary Configuration (`package.json:6-8`)
```json
"bin": {
  "scud": "bin/scud.js"
}
```

#### Postinstall Hook (`package.json:10`)
```json
"scripts": {
  "postinstall": "node bin/postinstall.js"
}
```

#### Binary Resolution in Wrapper (`bin/scud.js:165-178`)
For Rust commands:
1. `~/.cargo/bin/scud` (cargo-installed)
2. `<package>/scud-cli/target/release/scud` (local release build)
3. `<package>/scud-cli/target/debug/scud` (local debug build)

### Rust CLI Commands Enum

Full list from `main.rs:67-350` (33 commands):
- `Init` (line 69) - **exists in Rust**
- `Tags` (line 76)
- `List` with alias `view` (line 82) - **not the browser view**
- `Show` (line 102)
- `SetStatus` (line 112)
- `Next` (line 124)
- `Stats` (line 135)
- `Migrate` (line 142)
- `Waves` (line 149)
- `Config` (line 164) with nested `Agents` subcommand
- `Parse` (line 171)
- `Clean` (line 189)
- `AnalyzeComplexity` (line 200)
- `Expand` (line 211)
- `ReanalyzeDeps` (line 226)
- `Assign` (line 245)
- `WhoIs` (line 259)
- `NextBatch` (line 266)
- `Convert` (line 277)
- `Doctor` (line 292)
- `Mermaid` (line 307)
- `Log` (line 318)
- `LogShow` (line 331)
- `Warmup` (line 337)
- `Commit` (line 340)

## Code References

### Node.js Wrapper
- `bin/scud.js:13-14` - Command parsing
- `bin/scud.js:17-25` - Command classification
- `bin/scud.js:96-103` - `init()` function
- `bin/scud.js:105-112` - `install()` function
- `bin/scud.js:154-160` - `view` command interception
- `bin/scud.js:163-189` - Rust binary resolution and delegation
- `bin/scud.js:218-266` - `runView()` function
- `bin/scud.js:273-376` - `computeWaves()` (Kahn's algorithm)
- `bin/scud.js:382-644` - SCG parsing functions
- `bin/scud.js:669-1820` - HTML/CSS/JS generation

### Installation Scripts
- `bin/install.js:150-403` - `initProject()` function
- `bin/install.js:134-148` - `copyScudCommands()` function
- `bin/postinstall.js:16-36` - Platform detection
- `bin/postinstall.js:38-103` - Binary download
- `bin/postinstall.js:105-121` - Fallback source build

### Rust CLI
- `scud-cli/src/main.rs:67-350` - Commands enum
- `scud-cli/src/main.rs:82-100` - List command with `view` alias
- `scud-cli/src/commands/init.rs:10-108` - Init implementation
- `scud-cli/src/commands/list.rs:103-176` - List implementation
- `scud-cli/src/commands/config.rs:484-697` - Agent installation

### Package Configuration
- `package.json:6-8` - Binary entry point
- `package.json:10` - Postinstall hook
- `scud-cli/Cargo.toml:14-15` - Rust binary name

## Architecture Documentation

### Current Hybrid Flow

```
User runs: scud <command>
    |
    v
bin/scud.js (Node.js entry point)
    |
    +-- Is it 'view'? --> runView() [Node.js HTML generation]
    |
    +-- Is it 'init'? --> bin/install.js [Node.js project setup]
    |
    +-- Is it 'install'? --> bin/install.js [Node.js command copying]
    |
    +-- Is it in rustCommands? --> spawnSync(scudBinary, [command, ...args])
    |                                   |
    |                                   v
    |                              scud-cli/target/release/scud (Rust binary)
    |
    +-- Otherwise --> help() or switch default
```

### Gap Analysis for Rust-Only Architecture

| Command | Node.js | Rust CLI | Gap |
|---------|---------|----------|-----|
| `init` | Full implementation | Partial (no slash command copying) | Port slash command installation |
| `view` | Full HTML viewer | Alias for `list` | Port entire HTML viewer (~1600 lines) |
| `install` | Full implementation | None | Create new command |

### Static Webpage View Implementation Details

The view consists of:
1. **SCG Parser** (lines 382-644): Parses `.scg` file format
2. **Wave Computer** (lines 273-376): Kahn's algorithm for topological sort
3. **HTML Generator** (lines 669-740): Document structure
4. **CSS Styles** (lines 745-1202): ~450 lines of styling
5. **Client JavaScript** (lines 1207-1809): ~600 lines
   - Mermaid diagram rendering
   - Tab navigation
   - Phase selector
   - Pan/zoom functionality
   - Task detail panel
   - Wave view rendering
   - Stats dashboard

External dependency: `open` npm package (v10.0.3) for launching browser

## Open Questions

1. **Should the HTML viewer be a separate Rust crate or embedded in scud-cli?**
   - Rust options: `webbrowser` crate for opening browser, templates via `askama` or string literals

2. **How to handle the `open` package equivalent in Rust?**
   - `webbrowser` crate provides cross-platform browser opening

3. **Should Mermaid.js be bundled or loaded from CDN?**
   - Current implementation uses CDN (`cdn.jsdelivr.net`)

4. **How to distribute slash command templates with Rust binary?**
   - Options: embed in binary via `include_str!`, download from URL, or separate asset package

5. **What happens to users who have the Node.js wrapper installed?**
   - Migration strategy needed for existing installations
