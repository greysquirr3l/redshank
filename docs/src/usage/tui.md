# TUI Guide

Launch the TUI with `redshank tui`.

## Layout

```
┌─────────────┬──────────────────────────────┬──────────────┐
│  Sessions   │          Chat Log            │ Wiki Graph   │
│             │                              │              │
│ > Session 1 │  [user] Investigate...       │  Entity A    │
│   Session 2 │  [agent] I'll start with...  │    │         │
│             │  [tool] fec_filings result   │  Entity B    │
│             │                              │              │
├─────────────┴──────────────────────────────┴──────────────┤
│ > _                                                        │
└────────────────────────────────────────────────────────────┘
```

- **Sessions** (left) — sidebar listing saved sessions. Navigate with `↑`/`↓`, select with `Enter`.
- **Chat Log** (center) — scrolling log of the agent conversation. Scroll with `↑`/`↓` or `PgUp`/`PgDn`.
- **Wiki Graph** (right) — character-cell canvas rendering the petgraph entity DAG. Nodes are entity names; edges are relationships discovered during the investigation.
- **Input** (bottom) — type an objective or slash command and press `Enter`.

## Slash commands

| Command | Description |
|---------|-------------|
| `/model <name>` | Switch model for the current session |
| `/effort <low\|medium\|high>` | Change reasoning effort |
| `/new` | Start a new session |
| `/sessions` | List all sessions |
| `/resume <id>` | Resume a session by ID |
| `/export <path>` | Export current wiki to a directory |
| `/quit` or `q` | Exit the TUI |
