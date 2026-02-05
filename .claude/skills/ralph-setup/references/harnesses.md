# AI Harnesses for Ralph

Ralph works with multiple AI coding CLIs. Choose based on your needs.

## Claude Code (Recommended)

Anthropic's official CLI for Claude.

```bash
# Install
npm install -g @anthropic-ai/claude-code

# Usage
./loop.sh --harness claude --plan
./loop.sh --harness claude --build --allow-subtasks
```

**Pros:**
- Best agentic capabilities
- Native subagent support (Task tool)
- Fine-grained tool permissions
- Extended thinking (Ultrathink)

**Tool permissions by mode:**

| Mode | Allowed Tools |
|------|---------------|
| Plan | Read, Glob, Grep, Task, WebFetch, WebSearch |
| Build | Edit, Write, Bash, Read, Glob, Grep |
| Build + subtasks | Edit, Write, Bash, Read, Glob, Grep, Task |
| Dangerous | All tools, no prompts |

---

## OpenCode

Multi-model agentic coding CLI.

```bash
# Install
pip install opencode

# Usage
./loop.sh --harness opencode --model gpt-4-turbo
./loop.sh --harness opencode --model claude-3-opus
```

**Available models:**
- OpenAI: `gpt-4`, `gpt-4-turbo`, `gpt-4o`
- Anthropic: `claude-3-opus`, `claude-3-sonnet`, `claude-3-haiku`
- Google: `gemini-pro`, `gemini-ultra`

**Pros:**
- Model flexibility
- Works with multiple providers
- Good for A/B testing models

**Cons:**
- Less sophisticated permission system
- No native subagent support

---

## Codex CLI

OpenAI's coding agent CLI.

```bash
# Install
npm install -g @openai/codex

# Usage
./loop.sh --harness codex --model o1
./loop.sh --harness codex --model o1-mini
```

**Available models:**
- `o1` - Full reasoning model (slower, more thorough)
- `o1-mini` - Faster reasoning model
- `gpt-4`, `gpt-4-turbo`

**Approval modes:**
- `suggest` - Show changes, require approval (plan mode)
- `auto-edit` - Auto-approve file edits (build mode)
- `full-auto` - Auto-approve everything (dangerous mode)

**Pros:**
- Good reasoning with o1 models
- Native sandbox support

**Cons:**
- No subagent support
- Slower with o1

---

## Custom Harness

Use any CLI that accepts a prompt file.

```bash
# In ralph.conf:
HARNESS="custom"
CUSTOM_CMD="my-agent run --file {PROMPT_FILE} --auto-approve"

# Usage
./loop.sh
```

**Requirements:**
- Must accept prompt via file or stdin
- Must exit with code 0 on success
- Must modify files in working directory
- Should support git operations

**Example custom commands:**

```bash
# Aider
CUSTOM_CMD="aider --file {PROMPT_FILE} --yes"

# Continue
CUSTOM_CMD="continue --prompt-file {PROMPT_FILE}"

# Custom wrapper
CUSTOM_CMD="./my-wrapper.sh {PROMPT_FILE}"
```

---

## Comparison Matrix

| Feature | Claude Code | OpenCode | Codex | Custom |
|---------|-------------|----------|-------|--------|
| Subagents | Yes (Task) | No | No | Depends |
| Tool permissions | Fine-grained | Basic | Approval modes | Depends |
| Extended thinking | Yes | No | o1 only | Depends |
| Model flexibility | Claude only | Multi-model | OpenAI only | Any |
| Sandbox support | Yes | Basic | Yes | Depends |

## Recommendation

1. **Start with Claude Code** - Best agentic capabilities
2. **Try OpenCode with GPT-4** if you need different models
3. **Use Codex with o1** for complex reasoning tasks
4. **Custom** for specialized setups or proprietary tools

## Configuration

All harnesses are configured via `ralph.conf`:

```bash
HARNESS="claude"           # Which CLI
MODEL="gpt-4"              # Model (if needed)
CUSTOM_CMD=""              # Custom command template
ALLOW_SUBTASKS=true        # Enable Task tool (Claude only)
```

Or via command line:

```bash
./loop.sh --harness opencode --model gpt-4
./loop.sh --harness claude --allow-subtasks
```
