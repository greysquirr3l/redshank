# T22 — ratatui TUI: chat pane, wiki-graph canvas, activity indicator, REPL

> **Depends on**: T-agent-engine, T-wiki-graph, T-session-store.

## Goal

Implement the interactive TUI in redshank-tui using ratatui + crossterm.
Three-pane layout: sidebar (sessions + model), chat log, wiki-graph canvas.
Activity indicator, slash-command REPL, streaming content rendering.


## Project Context

- Project: `redshank` — Redshank is an autonomous recursive language-model investigation agent written
in Rust 1.94 (edition 2024). It ingests heterogeneous public datasets — campaign
finance, lobbying disclosures, federal contracts, corporate registries,
sanctions lists (OFAC, UN, EU, World Bank), property records, nonprofit
filings, corporate registries (GLEIF, OpenCorporates, FinCEN BOI, state SOS
portals), federal courts (RECAP/CourtListener), individual-person OSINT
(breach exposure, username enumeration across 300+ platforms, voter rolls,
github profiles, WHOIS history, patent/trademark inventors), and media
intelligence (GDELT) — resolves entities across all of them, and surfaces
non-obvious connections through evidence-backed analysis written into a live
knowledge-graph wiki.

The agent runs a tool-calling loop that can recursively delegate subtasks to
child agent invocations, condense context on long runs, apply a cheap judge
model to evaluate acceptance criteria, and stream its reasoning to an interactive
ratatui TUI. Web fetches use stygian-graph pipelines (with optional stygian-browser
anti-detection automation for JS-rendered pages). A compiled binary ships as a
single executable with no Python or Node.js runtime dependency.

- Language: rust
- Architecture: hexagonal-ddd-cqrs-security-first



## Strategy: TDD (Red-Green-Refactor)

### 1. RED — Write failing tests first

- Test: ActivityIndicator transitions Idle→Thinking→Running→Streaming→Idle on correct events.
- Test: slash command parser extracts '/model claude-opus-4-6 --save' correctly.
- Test: WikiGraphCanvas re-renders after WikiChanged event (node count increases).
- Test: AppEvent channel delivers ContentDelta events in order.


### 2. GREEN — Implement to pass

- Layout: Vertical [ Header | Horizontal [ Sidebar(20%) | Chat(55%) | Graph(25%) ] | Footer ].
- Sidebar: scrollable session list, current provider + model display, reasoning effort badge.
- Chat pane: scrolling RichLog of agent turns; user input rendered in a styled InputBox at bottom; markdown-ish rendering (bold, code blocks, bullet lists) via ratatui Paragraph.
- WikiGraphCanvas: character-cell petgraph force-layout rendering; nodes as coloured 3-char boxes; edges as ASCII lines; category colours as in wiki-graph phase.
- ActivityIndicator: 8fps tick; states: Thinking(elapsed), Running(tool_name, elapsed), Streaming(elapsed, partial_text_preview).
- Agent runs in a background tokio task; events sent to TUI via mpsc::channel with AppEvent enum: ContentDelta(String), ToolStart(String), ToolEnd(String, String), AgentComplete(String), WikiChanged.
- Slash commands: /model [name] [--save], /model list, /reasoning [low|medium|high|off], /status, /clear, /quit, /help.
- Non-TUI fallback: --no-tui flag renders plain streaming output to stdout (for scripting / CI).
- Banner on startup: figlet-style 'redshank' in a ratatui Paragraph using block-character art.


### 3. REFACTOR — Clean up while green

- Remove duplication
- Improve naming and structure
- Keep all tests passing


## Housekeeping: TODO / FIXME Sweep

Before running preflight, scan all files you created or modified in this task for
`TODO`, `FIXME`, `HACK`, `XXX`, and similar markers.

- **Resolve** any that fall within the scope of this task's goal.
- **Leave in place** any that reference work belonging to a later task or phase — but ensure they include a task reference (e.g. `// TODO(T07): wire up auth adapter`).
- **Remove** any placeholder markers that are no longer relevant after your implementation.

If none are found, move on.

## Preflight

```bash
cargo build --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

## Exit Criteria

- [ ] All code compiles without errors or warnings
- [ ] All tests pass
- [ ] Linter passes with no warnings
- [ ] Implementation matches the goal described above
- [ ] No unresolved TODO/FIXME/HACK markers that belong to this task's scope

## After Completion

Update PROGRESS.md row for T22 to `[x]`.
Commit: `feat(tui-ratatui): implement ratatui tui: chat pane, wiki-graph canvas, activity indicator, repl`
