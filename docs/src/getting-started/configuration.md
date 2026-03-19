# Configuration

## Credential resolution order

Redshank merges credentials from all sources, with earlier sources taking priority:

1. Process environment variables (e.g. `ANTHROPIC_API_KEY`)
2. `.env` file in the current working directory
3. `<workspace>/.redshank/credentials.json`
4. `~/.redshank/credentials.json` (user-level fallback)

Credential files are written `chmod 600`. Keys never appear in log output at any level.

## Environment variables

Both bare keys and the legacy `OPENPLANTER_` prefix are accepted.

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Anthropic Claude |
| `OPENAI_API_KEY` | OpenAI |
| `OPENROUTER_API_KEY` | OpenRouter |
| `CEREBRAS_API_KEY` | Cerebras |
| `OLLAMA_BASE_URL` | Local Ollama instance URL |
| `EXA_API_KEY` | Exa neural search |
| `VOYAGE_API_KEY` | Voyage AI embeddings |
| `HIBP_API_KEY` | Have I Been Pwned breach data |
| `GITHUB_TOKEN` | GitHub API (profile fetcher) |

Copy `.env.example` from the repo root and fill in the keys you need:

```bash
cp .env.example .env
chmod 600 .env
```

## credentials.json

For persistent storage, copy `credentials.example.json` to `.redshank/credentials.json`:

```bash
mkdir -p .redshank
cp credentials.example.json .redshank/credentials.json
chmod 600 .redshank/credentials.json
```

The file format maps directly to the environment variable names (snake_case):

```json
{
  "anthropic_api_key": "sk-ant-...",
  "openai_api_key": "sk-...",
  "openrouter_api_key": "sk-or-...",
  "cerebras_api_key": "...",
  "ollama_base_url": "http://localhost:11434",
  "exa_api_key": "...",
  "voyage_api_key": "...",
  "hibp_api_key": "...",
  "github_token": "ghp_..."
}
```

All fields are optional. Unknown keys are silently ignored.

## settings.json

Persistent settings live in `<workspace>/.redshank/settings.json`. Copy `settings.example.json` to get started:

```bash
cp settings.example.json .redshank/settings.json
```

```json
{
  "default_model": "claude-sonnet-4-20250514",
  "default_reasoning_effort": "medium",
  "default_model_anthropic": "claude-sonnet-4-20250514",
  "default_model_openai": "gpt-4o",
  "default_model_openrouter": "anthropic/claude-sonnet-4",
  "default_model_cerebras": "llama-3.3-70b",
  "default_model_ollama": "llama3.2"
}
```

`default_reasoning_effort` accepts `low`, `medium`, or `high`. Per-provider model names override the global `default_model` fallback for that provider only.
