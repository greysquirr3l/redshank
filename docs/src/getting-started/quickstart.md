# Quickstart

## 1. Set credentials

At minimum, set one LLM provider key. Redshank resolves credentials from environment variables, a `.env` file in the working directory, or `~/.redshank/credentials`.

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

## 2. Launch the TUI

```bash
redshank tui
```

The TUI opens with a session sidebar on the left, chat log in the center, and a wiki-graph canvas on the right. Type your investigation objective at the prompt and press Enter.

## 3. Or run a one-shot investigation

```bash
redshank run "Who are the top donors to PACs linked to defense contractors with active SAM.gov registrations?"
```

The agent runs autonomously, calling fetchers and writing findings to `wiki/` in the working directory.

## 4. Inspect the wiki

```bash
ls wiki/
cat wiki/index.md
```

Each entity gets its own Markdown file. Cross-references between entities are tracked in the petgraph DAG and rendered in the TUI canvas.

## Next steps

- [Configuration](./configuration.md) — set default models, reasoning effort, and workspace options
- [CLI Reference](../usage/cli.md) — full flag reference for `redshank run`, `redshank fetch`, and more
- [Running Investigations](../usage/investigations.md) — strategies for complex multi-source investigations
