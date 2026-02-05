#!/bin/bash
# Ralph Wiggum Outer Loop
# Runs AI agent in a loop with fresh context each iteration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# -----------------------------------------------------------------------------
# Default Configuration (override via ralph.conf or environment)
# -----------------------------------------------------------------------------
PROMPT_FILE="${PROMPT_FILE:-PROMPT.md}"
MAX_ITERATIONS="${MAX_ITERATIONS:-100}"
SLEEP_BETWEEN="${SLEEP_BETWEEN:-5}"

# Harness: claude | opencode | codex | custom
HARNESS="${HARNESS:-claude}"

# Model (required for opencode/codex, ignored for claude)
MODEL="${MODEL:-}"

# Custom command template (for HARNESS=custom)
# Use {PROMPT_FILE} as placeholder
CUSTOM_CMD="${CUSTOM_CMD:-}"

# Mode: plan | build
MODE="${MODE:-build}"

# Allow subtasks in build mode (true/false)
ALLOW_SUBTASKS="${ALLOW_SUBTASKS:-false}"

# -----------------------------------------------------------------------------
# Load config file if exists
# -----------------------------------------------------------------------------
if [ -f "ralph.conf" ]; then
    echo -e "${BLUE}Loading ralph.conf...${NC}"
    source ralph.conf
fi

# -----------------------------------------------------------------------------
# Parse command line arguments
# -----------------------------------------------------------------------------
usage() {
    cat << EOF
Usage: ./loop.sh [OPTIONS]

Options:
  --plan                Run in planning mode (read-only analysis)
  --build               Run in building mode (default)
  --allow-subtasks      Allow Task tool for spawning subagents (build mode)
  --harness <name>      AI harness: claude, opencode, codex, custom
  --model <model>       Model to use (required for opencode/codex)
  --max <n>             Maximum iterations (default: 100)
  --sleep <n>           Seconds between iterations (default: 5)
  --prompt <file>       Prompt file to use (default: PROMPT.md)
  --dangerous           Skip all permission checks (use with caution)
  -h, --help            Show this help

Examples:
  ./loop.sh --plan                           # Planning mode with Claude Code
  ./loop.sh --build --allow-subtasks         # Build mode with subtasks enabled
  ./loop.sh --harness opencode --model gpt-4 # Use OpenCode with GPT-4
  ./loop.sh --harness codex --model o1       # Use Codex with o1

Environment variables (or set in ralph.conf):
  HARNESS, MODEL, MODE, ALLOW_SUBTASKS, MAX_ITERATIONS, SLEEP_BETWEEN, PROMPT_FILE
EOF
    exit 0
}

DANGEROUS_MODE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --plan)
            MODE="plan"
            PROMPT_FILE="PROMPT_plan.md"
            shift
            ;;
        --build)
            MODE="build"
            PROMPT_FILE="PROMPT_build.md"
            shift
            ;;
        --allow-subtasks)
            ALLOW_SUBTASKS=true
            shift
            ;;
        --harness)
            HARNESS="$2"
            shift 2
            ;;
        --model)
            MODEL="$2"
            shift 2
            ;;
        --max)
            MAX_ITERATIONS="$2"
            shift 2
            ;;
        --sleep)
            SLEEP_BETWEEN="$2"
            shift 2
            ;;
        --prompt)
            PROMPT_FILE="$2"
            shift 2
            ;;
        --dangerous)
            DANGEROUS_MODE=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            ;;
    esac
done

# -----------------------------------------------------------------------------
# Validate configuration
# -----------------------------------------------------------------------------
if [[ ! -f "$PROMPT_FILE" ]]; then
    echo -e "${RED}Error: Prompt file not found: $PROMPT_FILE${NC}"
    exit 1
fi

if [[ "$HARNESS" == "opencode" || "$HARNESS" == "codex" ]] && [[ -z "$MODEL" ]]; then
    echo -e "${RED}Error: --model is required for $HARNESS harness${NC}"
    exit 1
fi

if [[ "$HARNESS" == "custom" ]] && [[ -z "$CUSTOM_CMD" ]]; then
    echo -e "${RED}Error: CUSTOM_CMD must be set for custom harness${NC}"
    echo "Set it in ralph.conf or environment"
    exit 1
fi

# -----------------------------------------------------------------------------
# Build the command based on harness and mode
# -----------------------------------------------------------------------------
build_command() {
    local prompt_file="$1"

    case "$HARNESS" in
        claude)
            # Claude Code CLI
            local cmd="cat '$prompt_file' | claude"

            if [[ "$DANGEROUS_MODE" == "true" ]]; then
                cmd="$cmd --dangerously-skip-permissions"
            elif [[ "$MODE" == "plan" ]]; then
                # Plan mode: allow all read tools, no write tools needed
                cmd="$cmd --allowedTools 'Read,Glob,Grep,Task,WebFetch,WebSearch'"
            elif [[ "$MODE" == "build" ]]; then
                # Build mode: allow edit/write/bash
                if [[ "$ALLOW_SUBTASKS" == "true" ]]; then
                    cmd="$cmd --allowedTools 'Edit,Write,Bash,Read,Glob,Grep,Task'"
                else
                    cmd="$cmd --allowedTools 'Edit,Write,Bash,Read,Glob,Grep'"
                fi
            fi

            echo "$cmd"
            ;;

        opencode)
            # OpenCode CLI (agentic coding with various models)
            local cmd="opencode --model '$MODEL' --prompt-file '$prompt_file'"

            if [[ "$DANGEROUS_MODE" == "true" ]]; then
                cmd="$cmd --auto-approve"
            fi

            echo "$cmd"
            ;;

        codex)
            # Codex CLI
            local cmd="codex --model '$MODEL'"

            if [[ "$DANGEROUS_MODE" == "true" ]]; then
                cmd="$cmd --approval-mode full-auto"
            elif [[ "$MODE" == "plan" ]]; then
                cmd="$cmd --approval-mode suggest"
            else
                cmd="$cmd --approval-mode auto-edit"
            fi

            cmd="$cmd '$prompt_file'"
            echo "$cmd"
            ;;

        custom)
            # Custom command - replace {PROMPT_FILE} placeholder
            echo "${CUSTOM_CMD//\{PROMPT_FILE\}/$prompt_file}"
            ;;

        *)
            echo -e "${RED}Unknown harness: $HARNESS${NC}" >&2
            exit 1
            ;;
    esac
}

# -----------------------------------------------------------------------------
# Main loop
# -----------------------------------------------------------------------------
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║              Ralph Wiggum Autonomous Loop                    ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}Configuration:${NC}"
echo "  Harness:        $HARNESS"
[[ -n "$MODEL" ]] && echo "  Model:          $MODEL"
echo "  Mode:           $MODE"
echo "  Prompt file:    $PROMPT_FILE"
echo "  Allow subtasks: $ALLOW_SUBTASKS"
echo "  Max iterations: $MAX_ITERATIONS"
echo "  Sleep between:  ${SLEEP_BETWEEN}s"
[[ "$DANGEROUS_MODE" == "true" ]] && echo -e "  ${RED}DANGEROUS MODE: All permissions skipped${NC}"
echo ""

iteration=0

while [ $iteration -lt $MAX_ITERATIONS ]; do
    iteration=$((iteration + 1))
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}  Iteration ${iteration} of ${MAX_ITERATIONS}${NC}"
    echo -e "${GREEN}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""

    # Build and run the command
    cmd=$(build_command "$PROMPT_FILE")
    echo -e "${BLUE}Running: ${cmd}${NC}"
    echo ""

    if ! eval "$cmd"; then
        echo -e "${RED}Agent exited with error${NC}"
        echo "Sleeping before retry..."
        sleep "$SLEEP_BETWEEN"
        continue
    fi

    # Push changes if any
    if [ -n "$(git status --porcelain)" ]; then
        echo ""
        echo -e "${YELLOW}Pushing changes...${NC}"
        git push || echo -e "${RED}Push failed, continuing...${NC}"
    else
        echo ""
        echo "No changes to push"
    fi

    echo ""
    echo -e "${GREEN}Iteration ${iteration} complete${NC}"
    echo "Sleeping ${SLEEP_BETWEEN}s before next iteration..."
    sleep "$SLEEP_BETWEEN"
done

echo ""
echo -e "${YELLOW}Reached max iterations (${MAX_ITERATIONS})${NC}"
