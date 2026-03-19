# CLI Reference

## `redshank run`

Run a one-shot investigation.

```text
redshank run [OPTIONS] <OBJECTIVE>
```text

| Option | Default | Description |
|--------|---------|-------------|
| `--model <MODEL>` | settings default | Model name (e.g. `claude-opus-4-5`, `gpt-4o`) |
| `--depth <N>` | `3` | Maximum subtask recursion depth |
| `--steps <N>` | `50` | Step budget per agent invocation |
| `--workspace <PATH>` | `.` | Working directory for file tools and wiki output |
| `--effort <EFFORT>` | `high` | Reasoning effort: `low`, `medium`, `high` |
| `--replay <FILE>` | — | Replay a JSONL log file instead of calling the model |

## `redshank tui`

Launch the interactive TUI.

```text
redshank tui [OPTIONS]
```text

| Option | Default | Description |
|--------|---------|-------------|
| `--model <MODEL>` | settings default | Model to use for new sessions |
| `--session <ID>` | — | Resume an existing session by ID |

## `redshank fetch`

Run a standalone data fetcher and write NDJSON to stdout.

```text
redshank fetch <FETCHER> [OPTIONS]
```text

See [Data Sources](../data-sources/overview.md) for available fetcher names and their options.

## `redshank session`

Manage saved sessions.

```text
redshank session list
redshank session show <ID>
redshank session delete <ID>
```text

## `redshank configure`

Read and write persistent settings.

```text
redshank configure get <KEY>
redshank configure set <KEY> <VALUE>
redshank configure credentials
```text

## `redshank version`

Print the version and build info.
