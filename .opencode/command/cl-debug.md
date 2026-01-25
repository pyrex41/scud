---
description: Investigate issues during manual testing without modifying files
---

# Debug

You are tasked with investigating issues encountered during manual testing. This is a read-only investigation command.

## Getting Started

**If given a plan/ticket file:**
Acknowledge the file and ask: "What specifically went wrong? What were you testing?"

**If invoked without parameters:**
Ask: "What issue are you encountering? Please describe:
- What you were trying to do
- What happened instead
- When did it last work correctly (if known)"

## Available Investigation Tools

### Logs
- Application logs in standard locations
- Service-specific logs
- Error logs and stack traces

### Database (if applicable)
- Query current state
- Check recent events/transactions
- Verify data consistency

### Git State
- Current branch and commits
- Uncommitted changes
- Recent history

## Investigation Process

### Step 1: Understand the Problem
- Read any context provided (plan, ticket, description)
- Check git status for current state
- Identify what should be happening vs. what is happening

### Step 2: Investigate in Parallel
Spawn parallel task agents to examine:
- **Logs**: Look for errors, warnings, unexpected patterns
- **State**: Check database or file state
- **Code**: Verify the implementation matches expectations

### Step 3: Present Findings

Structure your debug report:

```
## Debug Report

### Problem Summary
[What went wrong in one sentence]

### Evidence Found

#### From Logs
- [Relevant log entries with timestamps]
- [Error messages or stack traces]

#### From State
- [Database state or file contents]
- [Expected vs. actual values]

#### From Code
- [Relevant code paths]
- [Any mismatches between implementation and expectation]

### Root Cause Analysis
[Most likely explanation based on evidence]

### Recommended Next Steps
1. [First thing to try]
2. [Alternative if that doesn't work]
3. [Escalation path if needed]

### What I Couldn't Check
- [Things outside investigation scope]
- [Missing access or tools]
```

## Important Constraints

- **This is investigation only** - no file editing
- **Focus on evidence** - don't speculate without data
- **Be systematic** - check logs, state, and code
- **Present clearly** - make findings easy to act on

## What's Outside Scope

Some things you may not be able to investigate:
- Browser console errors (user needs to provide)
- Network requests (may need user assistance)
- External service internals
- Real-time state that's hard to capture

When you hit these limits, clearly explain what the user needs to check themselves.
