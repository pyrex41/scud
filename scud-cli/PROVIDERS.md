# LLM Provider Configuration

SCUD supports multiple LLM providers. You can configure your preferred provider during initialization or by editing the config file.

## Supported Providers

### xAI (Grok)
**Default Model:** `grok-code-fast-1`

```bash
# Initialize with xAI
scud init --provider xai

# Set API key
export XAI_API_KEY=your-xai-api-key
```

### Anthropic (Claude)
**Default Model:** `claude-sonnet-4-20250514`

```bash
# Initialize with Anthropic
scud init --provider anthropic

# Set API key
export ANTHROPIC_API_KEY=sk-ant-your-key
```

### OpenAI (GPT)
**Default Model:** `gpt-4-turbo`

```bash
# Initialize with OpenAI
scud init --provider openai

# Set API key
export OPENAI_API_KEY=sk-your-key
```

### OpenRouter
**Default Model:** `anthropic/claude-sonnet-4`

```bash
# Initialize with OpenRouter
scud init --provider openrouter

# Set API key
export OPENROUTER_API_KEY=sk-or-your-key
```

## Configuration File

The configuration is stored in `.scud/config.toml`:

```toml
[llm]
provider = "xai"
model = "grok-code-fast-1"
max_tokens = 4096
```

## Changing Providers

To change providers after initialization:

1. Edit `.scud/config.toml`
2. Update the `provider` and `model` fields
3. Set the appropriate API key environment variable

Or use the CLI:
```bash
scud config set-provider <provider> --model <model>
```

## Interactive Mode

If you don't specify a provider, SCUD will prompt you interactively:

```bash
scud init
# Select your LLM provider:
# > xAI (Grok)
#   Anthropic (Claude)
#   OpenAI (GPT)
#   OpenRouter
```

## API Endpoints

Each provider uses a different API endpoint:

- **xAI:** `https://api.x.ai/v1/chat/completions`
- **Anthropic:** `https://api.anthropic.com/v1/messages`
- **OpenAI:** `https://api.openai.com/v1/chat/completions`
- **OpenRouter:** `https://openrouter.ai/api/v1/chat/completions`

## Custom Models

You can override the default model by editing the config file:

```toml
[llm]
provider = "xai"
model = "grok-2-latest"  # Use a different model
max_tokens = 8192        # Increase token limit
```

## Environment Variables

SCUD reads API keys from environment variables for security:

| Provider | Environment Variable |
|----------|---------------------|
| xAI | `XAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |
| OpenAI | `OPENAI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

Set these in your shell profile (`.bashrc`, `.zshrc`, etc.) for persistence:

```bash
# Add to ~/.zshrc or ~/.bashrc
export XAI_API_KEY="your-key-here"
```

## Troubleshooting

### Authentication Error
```
Error: XAI_API_KEY environment variable not set
```

**Solution:** Set the appropriate API key for your provider.

### Invalid Provider
```
Error: Invalid provider: xxx. Valid options: xai, anthropic, openai, openrouter
```

**Solution:** Use one of the supported provider names.

### API Error
```
xai API error (401 Unauthorized): {"error": {"message": "Invalid API key"}}
```

**Solution:** Verify your API key is correct and has not expired.
