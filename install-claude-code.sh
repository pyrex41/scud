#!/bin/bash

# BMAD-TM Lite Installation Script for Claude Code CLI
# This script sets up the workflow orchestration system

set -e

echo "🚀 BMAD-TM Lite Installation for Claude Code CLI"
echo "=================================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Get project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo -e "${BLUE}Project root:${NC} $PROJECT_ROOT"
echo ""

# Check if Task Master CLI is installed
echo -e "${BLUE}Step 1: Checking Task Master CLI...${NC}"
if command -v task-master &> /dev/null; then
    TASKMASTER_VERSION=$(task-master --version 2>&1 || echo "unknown")
    echo -e "${GREEN}✓ Task Master CLI found${NC} ($TASKMASTER_VERSION)"
else
    echo -e "${RED}✗ Task Master CLI not found${NC}"
    echo ""
    echo "Install Task Master CLI:"
    echo "  npm install -g task-master"
    echo ""
    read -p "Would you like to install it now? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        npm install -g task-master
        echo -e "${GREEN}✓ Task Master CLI installed${NC}"
    else
        echo -e "${YELLOW}⚠ Skipping Task Master CLI installation${NC}"
        echo "You'll need to install it manually before using BMAD-TM Lite"
    fi
fi
echo ""

# Check if Node.js is installed (for validator)
echo -e "${BLUE}Step 2: Checking Node.js...${NC}"
if command -v node &> /dev/null; then
    NODE_VERSION=$(node --version)
    echo -e "${GREEN}✓ Node.js found${NC} ($NODE_VERSION)"
else
    echo -e "${RED}✗ Node.js not found${NC}"
    echo "Node.js is required for the Task Master validator."
    echo "Install from: https://nodejs.org/"
    exit 1
fi
echo ""

# Initialize Task Master if not already done
echo -e "${BLUE}Step 3: Initializing Task Master...${NC}"
cd "$PROJECT_ROOT"
if [ -f ".taskmaster/tasks/tasks.json" ]; then
    echo -e "${GREEN}✓ Task Master already initialized${NC}"
else
    mkdir -p .taskmaster/tasks
    echo '{}' > .taskmaster/tasks/tasks.json
    echo -e "${GREEN}✓ Task Master initialized${NC}"
fi
echo ""

# Create workflow state file
echo -e "${BLUE}Step 4: Creating workflow state...${NC}"
if [ -f ".taskmaster/workflow-state.json" ]; then
    echo -e "${YELLOW}⚠ Workflow state already exists${NC}"
    read -p "Overwrite? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}→ Keeping existing workflow state${NC}"
    else
        cp .taskmaster/workflow-state.json .taskmaster/workflow-state.json.backup
        echo -e "${YELLOW}→ Backed up to .taskmaster/workflow-state.json.backup${NC}"
        # Create fresh state
        cat > .taskmaster/workflow-state.json << 'EOF'
{
  "version": "1.0.0",
  "current_phase": "ideation",
  "active_epic": null,
  "phases": {
    "ideation": {
      "status": "active",
      "completed_at": null,
      "agent": "tm-pm",
      "description": "Product definition and PRD creation"
    },
    "planning": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-pm",
      "description": "Parse PRD into Task Master epics and tasks"
    },
    "architecture": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-architect",
      "description": "Technical design and architecture planning"
    },
    "implementation": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-dev",
      "description": "Task execution and development"
    },
    "retrospective": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-retrospective",
      "description": "Post-epic analysis and learning capture"
    }
  },
  "history": [],
  "completed_epics": [],
  "last_updated": null
}
EOF
        echo -e "${GREEN}✓ Workflow state created${NC}"
    fi
else
    cat > .taskmaster/workflow-state.json << 'EOF'
{
  "version": "1.0.0",
  "current_phase": "ideation",
  "active_epic": null,
  "phases": {
    "ideation": {
      "status": "active",
      "completed_at": null,
      "agent": "tm-pm",
      "description": "Product definition and PRD creation"
    },
    "planning": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-pm",
      "description": "Parse PRD into Task Master epics and tasks"
    },
    "architecture": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-architect",
      "description": "Technical design and architecture planning"
    },
    "implementation": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-dev",
      "description": "Task execution and development"
    },
    "retrospective": {
      "status": "pending",
      "completed_at": null,
      "agent": "tm-retrospective",
      "description": "Post-epic analysis and learning capture"
    }
  },
  "history": [],
  "completed_epics": [],
  "last_updated": null
}
EOF
    echo -e "${GREEN}✓ Workflow state created${NC}"
fi
echo ""

# Create directory structure
echo -e "${BLUE}Step 5: Creating directory structure...${NC}"
mkdir -p docs/prd
mkdir -p docs/epics
mkdir -p docs/architecture
mkdir -p docs/retrospectives
echo -e "${GREEN}✓ Directory structure created${NC}"
echo ""

# Copy slash commands to Claude Code directory
echo -e "${BLUE}Step 6: Installing slash commands...${NC}"
CLAUDE_COMMANDS_DIR="$HOME/.config/claude-code/commands"

if [ -d "$CLAUDE_COMMANDS_DIR" ]; then
    cp -r .claude/commands/* "$CLAUDE_COMMANDS_DIR/"
    echo -e "${GREEN}✓ Slash commands installed to $CLAUDE_COMMANDS_DIR${NC}"
    echo "  • /status"
    echo "  • /tm-pm"
    echo "  • /tm-architect"
    echo "  • /tm-dev"
    echo "  • /tm-retrospective"
else
    echo -e "${YELLOW}⚠ Claude Code commands directory not found${NC}"
    echo "  Expected: $CLAUDE_COMMANDS_DIR"
    echo ""
    echo "Options:"
    echo "  1. Symlink commands to your project (recommended):"
    echo "     ln -s $PROJECT_ROOT/.claude/commands ~/.config/claude-code/commands"
    echo ""
    echo "  2. Copy commands manually when Claude Code is installed"
fi
echo ""

# Make validator executable and add to PATH
echo -e "${BLUE}Step 7: Setting up Task Master validator...${NC}"
chmod +x "$PROJECT_ROOT/src/validators/taskmaster-validator.js"
echo -e "${GREEN}✓ Validator made executable${NC}"
echo ""
echo "To use the validator globally, add to your PATH:"
echo "  export PATH=\"\$PATH:$PROJECT_ROOT/src/validators\""
echo ""
echo "Or add this to your ~/.bashrc or ~/.zshrc:"
echo "  echo 'export PATH=\"\$PATH:$PROJECT_ROOT/src/validators\"' >> ~/.bashrc"
echo ""

# Test validator
echo -e "${BLUE}Step 8: Testing validator...${NC}"
if "$PROJECT_ROOT/src/validators/taskmaster-validator.js" get-command-availability &> /dev/null; then
    echo -e "${GREEN}✓ Validator working correctly${NC}"
else
    echo -e "${YELLOW}⚠ Validator test failed (may need Node.js modules)${NC}"
fi
echo ""

# Create .gitignore if it doesn't exist
echo -e "${BLUE}Step 9: Updating .gitignore...${NC}"
if [ ! -f ".gitignore" ]; then
    touch .gitignore
fi

# Add Task Master files to gitignore if not already present
if ! grep -q ".taskmaster/tasks/tasks.json" .gitignore; then
    echo "" >> .gitignore
    echo "# Task Master state (optional - depends on team workflow)" >> .gitignore
    echo "# .taskmaster/tasks/tasks.json" >> .gitignore
    echo "# .taskmaster/workflow-state.json" >> .gitignore
fi
echo -e "${GREEN}✓ .gitignore updated${NC}"
echo ""

# Installation complete
echo ""
echo -e "${GREEN}✅ BMAD-TM Lite installation complete!${NC}"
echo "=================================================="
echo ""
echo -e "${BLUE}Quick Start:${NC}"
echo ""
echo "  1. Check your workflow status:"
echo "     /status"
echo ""
echo "  2. Start your first epic:"
echo "     /tm-pm"
echo ""
echo "  3. Follow the workflow phases:"
echo "     Ideation → Planning → Architecture → Implementation → Retrospective"
echo ""
echo -e "${BLUE}Documentation:${NC}"
echo "  • Workflow Guide: src/workflows/workflow-plan-and-build.md"
echo "  • Quick Start: QUICKSTART.md"
echo ""
echo -e "${BLUE}Slash Commands Available:${NC}"
echo "  /status           - Show workflow status"
echo "  /tm-pm            - Product Manager (create PRD, plan epics)"
echo "  /tm-architect     - Architect (design technical solution)"
echo "  /tm-dev           - Developer (implement tasks)"
echo "  /tm-retrospective - Retrospective (capture learnings)"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "  • Read QUICKSTART.md for a guided walkthrough"
echo "  • Run /status to see your current workflow state"
echo "  • Start with /tm-pm when ready to create your first epic"
echo ""
echo "Happy building! 🚀"
echo ""
