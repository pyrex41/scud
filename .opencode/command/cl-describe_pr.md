---
description: Generate comprehensive PR descriptions following repository templates
---

# Generate PR Description

You are tasked with generating a comprehensive pull request description.

## Process

### Step 1: Identify the PR

- Check if current branch has a PR: `gh pr view --json url,number,title,state 2>/dev/null`
- If no PR exists, list open PRs: `gh pr list --limit 10 --json number,title,headRefName,author`
- Ask which PR to describe if unclear

### Step 2: Gather PR Information

- Get the full PR diff: `gh pr diff {number}`
- Get commit history: `gh pr view {number} --json commits`
- Get PR metadata: `gh pr view {number} --json url,title,number,state,baseRefName`

### Step 3: Analyze Changes Thoroughly

- Read through the entire diff carefully
- For context, read files referenced but not in the diff
- Understand the purpose and impact of each change
- Identify:
  - User-facing changes vs. internal implementation
  - Breaking changes or migration requirements
  - New dependencies or configurations

### Step 4: Run Verification

For any verification commands you can run:
- If it passes, mark checkbox: `- [x]`
- If it fails, keep unchecked with explanation: `- [ ]`
- If it requires manual testing, leave unchecked and note for user

### Step 5: Generate Description

Create a comprehensive description:

```markdown
## Summary

[2-3 sentences describing what this PR does and why]

## Changes

### [Category 1]
- Change description
- Another change

### [Category 2]
- Change description

## Breaking Changes

[List any breaking changes, or "None"]

## How to Verify

### Automated
- [x] `make check` passes
- [x] `make test` passes
- [ ] [Test that couldn't be run - reason]

### Manual
- [ ] [Manual verification step 1]
- [ ] [Manual verification step 2]

## Related

- Fixes #[issue number] (if applicable)
- Related to #[other PR] (if applicable)
```

### Step 6: Update the PR

- Update the PR description: `gh pr edit {number} --body-file [description file]`
- Confirm the update was successful
- Remind user of any unchecked verification steps

## Important Notes

- Be thorough but concise - descriptions should be scannable
- Focus on the "why" as much as the "what"
- Include breaking changes or migration notes prominently
- If PR touches multiple components, organize accordingly
- Always attempt to run verification commands when possible
- Clearly communicate which steps need manual testing
