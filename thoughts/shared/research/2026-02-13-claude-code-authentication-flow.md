---
date: 2026-02-13T14:48:54+0000
researcher: reuben
git_commit: bd040072d33d1f24f7c93be15591cb7db9483e7a
branch: main
repository: pi-mono
topic: "How the coding agent handles Claude Code authentication and uses the Claude Code subscription to call Claude models"
tags: [research, codebase, authentication, oauth, claude-code, anthropic, subscription]
status: complete
last_updated: 2026-02-13
last_updated_by: reuben
---

# Research: Claude Code Authentication & Subscription Model Calling

**Date**: 2026-02-13T14:48:54+0000
**Researcher**: reuben
**Git Commit**: bd040072d33d1f24f7c93be15591cb7db9483e7a
**Branch**: main
**Repository**: pi-mono

## Research Question
How does the coding agent handle Claude Code authentication and use the Claude Code subscription (Pro/Max) to call Claude models?

## Summary

The coding agent impersonates Claude Code when using Anthropic OAuth tokens (from Claude Pro/Max subscriptions). It does this by:

1. Performing an OAuth PKCE flow against `claude.ai/oauth/authorize` with a hardcoded client ID
2. Detecting OAuth tokens at call time by the `sk-ant-oat` prefix
3. When an OAuth token is detected, switching the Anthropic SDK client to Bearer auth, injecting Claude Code identity headers (version, user-agent, beta flags), prepending a Claude Code system prompt, and renaming tools to match Claude Code's canonical casing

This allows users with a Claude Pro/Max subscription to use their subscription billing to call Claude models through the coding agent, rather than needing a separate Anthropic API key.

## Detailed Findings

### 1. OAuth Login Flow (Acquiring Tokens)

**File**: `packages/ai/src/utils/oauth/anthropic.ts`

The Anthropic OAuth provider implements a PKCE authorization code flow:

- **Client ID**: Base64-encoded constant decoded at runtime (line 9)
- **Authorization URL**: `https://claude.ai/oauth/authorize` (line 10)
- **Token URL**: `https://console.anthropic.com/v1/oauth/token` (line 11)
- **Redirect URI**: `https://console.anthropic.com/oauth/code/callback` (line 12)
- **Scopes**: `org:create_api_key user:profile user:inference` (line 13)

The flow works as follows:
1. Generate PKCE verifier + challenge (`generatePKCE()`)
2. Build authorization URL with PKCE challenge and open in browser
3. User authenticates on claude.ai and gets a code in format `code#state`
4. Exchange code for `access_token` + `refresh_token` via POST to token URL
5. Store credentials with expiry (expires_in minus 5-minute buffer)

Token refresh uses the same token endpoint with `grant_type: refresh_token`.

The provider is registered as `"anthropic"` with display name `"Anthropic (Claude Pro/Max)"` (line 120-138).

### 2. Credential Storage & Resolution

**File**: `packages/coding-agent/src/core/auth-storage.ts`

The `AuthStorage` class manages all credentials in `~/.pi/agent/auth.json` with this priority order (line 275-340):

1. **Runtime override** — CLI `--api-key` flag (`setRuntimeApiKey()`)
2. **API key from auth.json** — stored as `{ type: "api_key", key: "..." }`
3. **OAuth token from auth.json** — stored as `{ type: "oauth", access: "...", refresh: "...", expires: N }`, auto-refreshed with file locking
4. **Environment variable** — `ANTHROPIC_OAUTH_TOKEN` or `ANTHROPIC_API_KEY`
5. **Fallback resolver** — custom provider keys from `models.json`

Token refresh uses `proper-lockfile` to prevent race conditions when multiple agent instances try to refresh simultaneously (line 180-273). After acquiring the lock, it re-reads the file to check if another instance already refreshed the token.

### 3. Environment Variable Precedence

**File**: `packages/ai/src/env-api-keys.ts:58-61`

For the `"anthropic"` provider, `ANTHROPIC_OAUTH_TOKEN` takes precedence over `ANTHROPIC_API_KEY`:

```typescript
if (provider === "anthropic") {
    return process.env.ANTHROPIC_OAUTH_TOKEN || process.env.ANTHROPIC_API_KEY;
}
```

### 4. OAuth Token Detection & Claude Code Identity

**File**: `packages/ai/src/providers/anthropic.ts`

The critical branching happens in `createClient()` (line 486-565). The function detects OAuth tokens by checking for the `sk-ant-oat` prefix (line 482-484):

```typescript
function isOAuthToken(apiKey: string): boolean {
    return apiKey.includes("sk-ant-oat");
}
```

When an OAuth token is detected, the client is created differently from a standard API key:

#### OAuth Token Path (line 526-546):
```typescript
const client = new Anthropic({
    apiKey: null,           // No x-api-key header
    authToken: apiKey,      // Bearer token in Authorization header
    baseURL: model.baseUrl,
    dangerouslyAllowBrowser: true,
    defaultHeaders: {
        "accept": "application/json",
        "anthropic-dangerous-direct-browser-access": "true",
        "anthropic-beta": "claude-code-20250219,oauth-2025-04-20,...",
        "user-agent": "claude-cli/2.1.2 (external, cli)",
        "x-app": "cli",
    },
});
```

#### Standard API Key Path (line 548-564):
```typescript
const client = new Anthropic({
    apiKey,                 // Standard x-api-key header
    baseURL: model.baseUrl,
    dangerouslyAllowBrowser: true,
    defaultHeaders: {
        "accept": "application/json",
        "anthropic-dangerous-direct-browser-access": "true",
        "anthropic-beta": "fine-grained-tool-streaming-2025-05-14,...",
    },
});
```

Key differences when using OAuth:
- Uses `authToken` (Bearer) instead of `apiKey` (x-api-key)
- Adds `claude-code-20250219` and `oauth-2025-04-20` beta flags
- Adds `user-agent: claude-cli/2.1.2 (external, cli)` header
- Adds `x-app: cli` header

### 5. System Prompt Injection for OAuth

**File**: `packages/ai/src/providers/anthropic.ts:581-606`

When OAuth is detected, a mandatory Claude Code identity system prompt is prepended:

```typescript
if (isOAuthToken) {
    params.system = [
        {
            type: "text",
            text: "You are Claude Code, Anthropic's official CLI for Claude.",
            ...(cacheControl ? { cache_control: cacheControl } : {}),
        },
    ];
    // Then append the actual system prompt after it
    if (context.systemPrompt) {
        params.system.push({
            type: "text",
            text: context.systemPrompt,
        });
    }
}
```

This is marked with the comment "For OAuth tokens, we MUST include Claude Code identity" (line 581).

### 6. Tool Name Normalization (Stealth Mode)

**File**: `packages/ai/src/providers/anthropic.ts:64-101`

When using OAuth, tools are renamed to match Claude Code's canonical casing. The code comments this as "Stealth mode: Mimic Claude Code's tool naming exactly" (line 64).

The canonical tool names (line 70-88):
```
Read, Write, Edit, Bash, Grep, Glob, AskUserQuestion, EnterPlanMode,
ExitPlanMode, KillShell, NotebookEdit, Skill, Task, TaskOutput,
TodoWrite, WebFetch, WebSearch
```

A case-insensitive lookup map converts tool names in both directions:
- **Outbound** (`toClaudeCodeName`): When sending tool definitions and tool_use blocks to the API, tool names are converted to Claude Code casing (line 93, used at line 740, 820)
- **Inbound** (`fromClaudeCodeName`): When receiving tool_use responses from the API, tool names are converted back to the agent's internal casing (line 94-101, used at line 281)

### 7. OAuth Provider Registry

**File**: `packages/ai/src/utils/oauth/index.ts`

Five OAuth providers are registered (line 45-51):
1. `anthropic` — Anthropic (Claude Pro/Max)
2. `github-copilot` — GitHub Copilot
3. `google-gemini-cli` — Google Cloud Code Assist (Gemini CLI)
4. `google-antigravity` — Antigravity (Google Cloud)
5. `openai-codex` — OpenAI Codex (ChatGPT Plus/Pro)

Each provider implements `OAuthProviderInterface` from `packages/ai/src/utils/oauth/types.ts`:
- `login(callbacks)` — Run OAuth flow, return credentials
- `refreshToken(credentials)` — Refresh expired tokens
- `getApiKey(credentials)` — Extract access token from credentials
- `modifyModels?(models, credentials)` — Optional: modify model configs (e.g., update baseUrl)

### 8. Model Registry Integration

**File**: `packages/coding-agent/src/core/model-registry.ts`

The `ModelRegistry` class ties everything together:

- `getAvailable()` (line 498-499): Filters models to only those with configured auth (uses `AuthStorage.hasAuth()`)
- `getApiKey(model)` (line 512-514): Delegates to `AuthStorage.getApiKey(model.provider)`, which handles OAuth refresh
- `isUsingOAuth(model)` (line 526-529): Checks if a model's provider has OAuth credentials
- `loadModels()` (line 260-286): After loading models, lets OAuth providers modify them via `modifyModels()` (e.g., GitHub Copilot uses this to update baseUrl based on credentials)

### 9. Version & Identity Constants

**File**: `packages/ai/src/providers/anthropic.ts:65`

The impersonated Claude Code version is hardcoded:
- **Version**: `2.1.2`
- **User-Agent**: `claude-cli/2.1.2 (external, cli)`
- **Beta Flags**: `claude-code-20250219,oauth-2025-04-20`
- **Tool name source**: https://cchistory.mariozechner.at/data/prompts-2.1.11.md

## Code References

- `packages/ai/src/providers/anthropic.ts:482-484` — OAuth token detection (`isOAuthToken`)
- `packages/ai/src/providers/anthropic.ts:486-565` — `createClient()` with OAuth vs API key branching
- `packages/ai/src/providers/anthropic.ts:567-649` — `buildParams()` with OAuth system prompt injection
- `packages/ai/src/providers/anthropic.ts:64-101` — Claude Code tool name normalization
- `packages/ai/src/providers/anthropic.ts:193-411` — `streamAnthropic()` main streaming function
- `packages/ai/src/utils/oauth/anthropic.ts:1-138` — Anthropic OAuth PKCE flow
- `packages/ai/src/utils/oauth/index.ts:45-51` — OAuth provider registry
- `packages/ai/src/utils/oauth/types.ts:34-52` — `OAuthProviderInterface`
- `packages/ai/src/env-api-keys.ts:58-61` — Environment variable precedence
- `packages/coding-agent/src/core/auth-storage.ts:40-348` — `AuthStorage` class (credential storage, refresh with locking)
- `packages/coding-agent/src/core/auth-storage.ts:275-340` — `getApiKey()` priority chain
- `packages/coding-agent/src/core/model-registry.ts:217-639` — `ModelRegistry` class
- `packages/coding-agent/src/core/model-registry.ts:526-529` — `isUsingOAuth()` check

## Architecture Documentation

### Authentication Flow (End-to-End)

```
User runs /login → OAuth selector → Anthropic selected
  → PKCE flow against claude.ai/oauth/authorize
  → Code exchanged at console.anthropic.com/v1/oauth/token
  → Credentials saved to ~/.pi/agent/auth.json as { type: "oauth", access, refresh, expires }

User sends message → agent-session → streamSimple()
  → AuthStorage.getApiKey("anthropic")
    → Checks runtime override → auth.json API key → auth.json OAuth (refresh if expired) → env var → fallback
  → Returns access token (sk-ant-oat...)

streamSimple() → streamAnthropic() with apiKey
  → createClient() detects sk-ant-oat prefix
  → Creates Anthropic SDK client with:
    - Bearer auth (authToken, not apiKey)
    - Claude Code beta flags
    - Claude Code user-agent
  → buildParams() adds "You are Claude Code" system prompt
  → convertTools() renames tools to Claude Code casing
  → API call to api.anthropic.com/v1/messages
  → Response tool names converted back from Claude Code casing
```

### Key Design Decisions

- OAuth tokens are detected by prefix (`sk-ant-oat`) rather than stored auth type, so environment variables (`ANTHROPIC_OAUTH_TOKEN`) also trigger Claude Code identity
- File locking prevents token refresh races across concurrent agent instances
- Tool name normalization is bidirectional: outbound to Claude Code casing, inbound back to internal casing
- The Claude Code identity (system prompt + headers) is mandatory for OAuth — the API requires it

## Open Questions

- How frequently does Anthropic update the required Claude Code version / beta flags, and how is the hardcoded version `2.1.2` kept in sync?
- What happens if the impersonated Claude Code version becomes too stale relative to the real Claude Code?
- The tool name source references `prompts-2.1.11.md` but the version constant is `2.1.2` — are these deliberately different?
