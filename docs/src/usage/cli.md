# CLI Reference

## `redshank run`

Run a one-shot investigation.

```text
redshank run [OPTIONS] <OBJECTIVE>
```

| Option | Default | Description |
|--------|---------|-------------|
| `-w, --workspace <PATH>` | current directory | Working directory for file tools and persisted state |
| `-m, --model <MODEL>` | `claude-sonnet-4-20250514` | Model name (for example `claude-opus-4-5`, `gpt-4o`) |
| `-r, --reasoning <LEVEL>` | `medium` | Reasoning effort: `off`, `low`, `medium`, `high` |
| `--no-tui` | `false` | Disable the interactive UI for command flows that support it |
| `--max-depth <N>` | `5` | Maximum recursion depth for sub-tasks |
| `--demo` | `false` | Enable demo mode with redacted entity names |

## `redshank tui`

Launch the interactive TUI.

```text
redshank tui [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `-w, --workspace <PATH>` | current directory | Workspace containing `.redshank/` state |
| `-m, --model <MODEL>` | `claude-sonnet-4-20250514` | Model to use for new sessions |
| `-r, --reasoning <LEVEL>` | `medium` | Initial reasoning effort |
| `--session <ID>` | — | Resume an existing session by ID |
| `--max-depth <N>` | `5` | Maximum recursion depth for spawned sub-tasks |
| `--demo` | `false` | Enable demo mode |

## `redshank fetch`

Run a standalone data fetcher.

```text
redshank fetch uk_corporate_intelligence --query <QUERY> [--output <DIR>]
redshank fetch uk-corporate-intelligence --query <QUERY> [--output <DIR>]
```

Currently the CLI fetch dispatcher wires the UK corporate intelligence fetcher only.

| Option | Default | Description |
|--------|---------|-------------|
| `--query <QUERY>` | — | Required search term or company name |
| `--output <DIR>` | current directory | Directory where the NDJSON output file will be written |

## `redshank session`

Manage saved sessions.

```text
redshank session list
redshank session resume <ID>
redshank session delete <ID>
```

## `redshank configure`

Launch the interactive credential setup wizard.

```text
redshank configure
redshank configure credentials
redshank setup
```

`redshank configure credentials` and `redshank setup` are compatibility aliases for the same interactive flow.

## `redshank version`

Print the version and build info.
