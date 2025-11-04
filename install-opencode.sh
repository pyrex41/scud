#!/bin/bash

# BMAD-TM Lite Installation Script for OpenCode
# This script sets up the workflow orchestration system for OpenCode

set -e

echo "🚀 BMAD-TM Lite Installation for OpenCode"
echo "=========================================="
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

# Create OpenCode skills directory
echo -e "${BLUE}Step 6: Creating OpenCode skills...${NC}"
mkdir -p .opencode/skills

# Create skill files from slash commands
cat > .opencode/skills/status.md << 'EOF'
# BMAD-TM Workflow Status Skill

Invoke this skill to show the current BMAD-TM workflow status.

## How to Use
User says: "show status" or "what's my workflow status?" or "status"

## Skill Behavior
Load and display:
- Current workflow phase
- Active epic and task progress
- Available commands
- Warnings or blockers
- Next steps guidance

Reference the full command documentation at: .claude/commands/status.md
EOF

cat > .opencode/skills/tm-pm.md << 'EOF'
# Product Manager Skill

Invoke this skill when the user wants to:
- Create a Product Requirements Document (PRD)
- Plan a new epic
- Break down requirements into tasks

## How to Use
User says: "I need to create a PRD" or "start product planning" or "tm-pm"

## Skill Behavior
1. Validate workflow phase (must be ideation or planning)
2. Load Product Manager agent persona from: .claude/commands/tm-pm.md
3. Follow the agent's workflow for current phase

Reference the full agent documentation at: .claude/commands/tm-pm.md
EOF

cat > .opencode/skills/tm-architect.md << 'EOF'
# Architect Skill

Invoke this skill when the user wants to:
- Design technical architecture
- Create technical specifications
- Enhance tasks with implementation details

## How to Use
User says: "design the architecture" or "create technical design" or "tm-architect"

## Skill Behavior
1. Validate workflow phase (must be architecture)
2. Validate active epic exists
3. Load Architect agent persona from: .claude/commands/tm-architect.md
4. Follow the agent's workflow

Reference the full agent documentation at: .claude/commands/tm-architect.md
EOF

cat > .opencode/skills/tm-dev.md << 'EOF'
# Developer Skill

Invoke this skill when the user wants to:
- Implement tasks from Task Master
- Write code following architecture
- Execute the development phase

## How to Use
User says: "start development" or "implement tasks" or "tm-dev"

## Skill Behavior
1. Validate workflow phase (must be implementation)
2. Validate active epic and architecture complete
3. Load Developer agent persona from: .claude/commands/tm-dev.md
4. Follow dependency-aware implementation workflow

Reference the full agent documentation at: .claude/commands/tm-dev.md
EOF

cat > .opencode/skills/tm-retrospective.md << 'EOF'
# Retrospective Skill

Invoke this skill when the user wants to:
- Conduct post-epic retrospective
- Capture learnings and insights
- Analyze completed work

## How to Use
User says: "run retrospective" or "review the epic" or "tm-retrospective"

## Skill Behavior
1. Validate all tasks in epic are complete
2. Load Retrospective agent persona from: .claude/commands/tm-retrospective.md
3. Follow retrospective workflow
4. Create comprehensive retrospective document

Reference the full agent documentation at: .claude/commands/tm-retrospective.md
EOF

echo -e "${GREEN}✓ OpenCode skills created${NC}"
echo "  • status"
echo "  • tm-pm"
echo "  • tm-architect"
echo "  • tm-dev"
echo "  • tm-retrospective"
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
echo -e "${GREEN}✅ BMAD-TM Lite installation for OpenCode complete!${NC}"
echo "========================================================="
echo ""
echo -e "${BLUE}Quick Start:${NC}"
echo ""
echo "  1. Tell OpenCode: 'show status'"
echo "     This will display your current workflow state"
echo ""
echo "  2. Start your first epic: 'start product planning'"
echo "     This will activate the Product Manager skill"
echo ""
echo "  3. Follow the workflow phases:"
echo "     Ideation → Planning → Architecture → Implementation → Retrospective"
echo ""
echo -e "${BLUE}Documentation:${NC}"
echo "  • Workflow Guide: src/workflows/workflow-plan-and-build.md"
echo "  • Quick Start: QUICKSTART.md"
echo ""
echo -e "${BLUE}Skills Available:${NC}"
echo "  status           - Show workflow status"
echo "  tm-pm            - Product Manager (create PRD, plan epics)"
echo "  tm-architect     - Architect (design technical solution)"
echo "  tm-dev           - Developer (implement tasks)"
echo "  tm-retrospective - Retrospective (capture learnings)"
echo ""
echo -e "${BLUE}How to Invoke Skills:${NC}"
echo "  Just describe what you want in natural language:"
echo "  • 'show me the current status'"
echo "  • 'I need to create a product requirements document'"
echo "  • 'let's design the architecture'"
echo "  • 'start implementing tasks'"
echo "  • 'run a retrospective'"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "  • Read QUICKSTART.md for a guided walkthrough"
echo "  • Tell OpenCode 'show status' to see your workflow state"
echo "  • Start with 'create a PRD' when ready for your first epic"
echo ""
echo "Happy building! 🚀"
echo ""
