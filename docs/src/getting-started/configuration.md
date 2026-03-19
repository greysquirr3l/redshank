# Configuration

## Credential resolution order

Redshank tries each source in order and uses the first value found:

1. Environment variable (e.g. `ANTHROPIC_API_KEY`)
2. `.env` file in the current working directory
3. `~/.redshank/credentials` (stored `chmod 600`)

Credential files are never written with broader permissions. Keys never appear in log output at any level.

## Supported credential keys

| Variable | Provider |
|----------|----------|
| `ANTHROPIC_API_KEY` | Anthropic Claude |
| `OPENAI_API_KEY` | OpenAI |
| `OPENROUTER_API_KEY` | OpenRouter |
| `CEREBRAS_API_KEY` | Cerebras |
| `FEC_API_KEY` | FEC bulk data |
| `HIBP_API_KEY` | Have I Been Pwned |
| `OPENCORPORATES_API_KEY` | OpenCorporates |

## Settings

Persistent settings live in `.redshank/settings.json` in the working directory:

```json
{
  "default_model": "claude-opus-4-5",
  "reasoning_effort": "high",
  "providers": {
    "anthropic": { "model": "claude-opus-4-5" },
    "openai": { "model": "gpt-4o" }
  }
}
```

You can update settings via the CLI:

```bash
redshank configure set default-model claude-sonnet-4-5
redshank configure set reasoning-effort medium
```
