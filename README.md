# Redshank

![RedShank Logo](./docs/assets/img/redshank-logo.png)

An autonomous recursive language-model investigation agent written in Rust.  Redshank ingests heterogeneous public datasets — campaign finance, lobbying disclosures, federal contracts, corporate registries, sanctions lists, court records, individual-person OSINT,and media intelligence — resolves entities across all of them, and surfaces non-obvious connections through evidence-backed analysis written into a live knowledge-graph wiki.

Redshank is a from-scratch Rust rewrite of [OpenPlanter](https://github.com/ShinMegamiBoson/OpenPlanter), replacing the Python runtime with a compiled binary that has zero Python or Node.js dependency.

## Installation

```bash
cargo install redshank-cli --locked
```

Or build from source:

```bash
git clone https://github.com/greysquirr3l/redshank.git
cd redshank
cargo build --release
```

The binary lands at `target/release/redshank`.

## Quickstart

```bash
# 1. Run the interactive credential wizard
redshank setup

# 2. Launch the TUI
redshank tui

# 3. Or run a one-shot investigation from the command line
redshank run "Who are the top donors to PACs linked to defense contractors with active SAM.gov registrations?"
```

`redshank configure` and `redshank configure credentials` invoke the same setup wizard.

## Features

- **Recursive tool-calling engine** — The agent loop calls tools, reads results,
  and can delegate subtasks to child agent invocations with independent context
  windows. Context condensation keeps long investigations under the token limit.
- **90+ fetcher modules** — Pull records from government databases, corporate
  registries, sanctions lists, court systems, and OSINT sources (see
  [Data Sources](#data-sources) below).
- **Knowledge-graph wiki** — Findings are written to interconnected Markdown
  documents with cross-references. A petgraph DAG tracks entities and
  relationships with fuzzy name matching.
- **Interactive TUI** — Three-pane ratatui interface: session sidebar, scrolling
  chat log, and a character-cell wiki-graph canvas. Slash commands for model
  switching, reasoning effort, and session management.
- **Multi-provider LLM support** — Anthropic (native Messages API with thinking
  budgets), OpenAI, OpenRouter, Cerebras, and Ollama (local).
- **Security-first architecture** — Fail-secure design with typed `AuthContext`,
  role-based `SecurityPolicy`, and `chmod 600` credential storage. Every data
  access path checks permissions before touching storage.
- **Stygian integration** — Optional `stygian` feature flag enables
  stygian-graph pipelines and stygian-browser anti-detection automation for
  JS-rendered pages.
- **Coraline MCP tools** — Optional `coraline` feature flag adds code-aware
  file reading, semantic search, repo mapping, and file editing via the
  Coraline MCP server.
- **CQRS + domain events** — Every mutating operation flows through idempotent
  Command handlers; every read through Query handlers. State transitions emit
  typed domain events persisted to SQLite.

## Data Sources

| Category | Fetchers |
| --- | --- |
| **Campaign Finance** | FEC filings, Senate lobbying disclosures, House lobbying disclosures |
| **Government Contracts** | USASpending, SAM.gov registrations, FPDS contract awards, federal audit clearinghouse |
| **Corporate Registries** | GLEIF (LEI lookups), OpenCorporates, FinCEN BOI, state Secretary of State portals, SEC EDGAR |
| **Financial** | FDIC institution search, ProPublica nonprofit 990 filings |
| **Sanctions** | OFAC SDN, UN consolidated sanctions, EU sanctions, World Bank debarred firms |
| **Environmental & Safety** | EPA ECHO compliance, OSHA inspection data |
| **Courts** | CourtListener (RECAP archive) |
| **Leaks & Offshore** | ICIJ offshore leaks database |
| **Individual OSINT** | HIBP breach exposure, GitHub profiles, Wayback Machine snapshots, WHOIS/RDAP history, voter registration, USPTO patent/trademark inventors, username enumeration (37 platforms), social media profiles |
| **Reference & Media** | Wikidata entity lookups, GDELT media monitoring, Census ACS demographics |
| **Property** | County property/assessor records |

## Configuration

Credentials are resolved in this priority order (first match wins):

1. `REDSHANK_<KEY>` — app-namespaced, useful when running multiple agents on one host
2. `OPENPLANTER_<KEY>` — legacy backward compatibility
3. `<KEY>` — bare env var (sufficient for most users)

Set keys as environment variables or in a `.env` file:

```bash
cp .env.example .env
# edit .env with your keys
chmod 600 .env
```

For persistent storage, copy the example credentials file:

```bash
mkdir -p .redshank
cp credentials.example.json .redshank/credentials.json
chmod 600 .redshank/credentials.json
```

Redshank merges from all sources in order: env vars → `.env` → `<workspace>/.redshank/credentials.json` → `~/.redshank/credentials.json`.

Model defaults live in `<workspace>/.redshank/settings.json`:

```json
{
  "default_model": "claude-sonnet-4-20250514",
  "default_reasoning_effort": "medium",
  "providers": {
    "OpenAiCompatible": {
      "enabled": true,
      "protocol": "openai_compatible",
      "deployment": "local",
      "base_url": "http://localhost:11434/v1",
      "default_model": "llama3.2"
    },
    "OpenAI": {
      "enabled": true,
      "protocol": "openai_compatible",
      "deployment": "local",
      "base_url": "http://localhost:1234/v1",
      "default_model": "qwen2.5-coder:latest"
    }
  }
}
```

Provider routing stays in `settings.json`, while actual secrets remain in
`.redshank/credentials.json`. That lets you point `OpenAI` at a local
OpenAI-compatible server, or route `Ollama` to a non-default host, without
copying API keys into general settings.

Override at runtime with CLI flags:

```bash
redshank run --model gpt-4o --reasoning high "Investigate ..."
```

## TUI Guide

Launch with `redshank tui` (or `redshank tui --session <id>` to resume).

| Area | Description |
| --- | --- |
| **Sidebar** (left 20%) | Session list. Select with arrow keys. |
| **Chat pane** (center 55%) | Scrolling conversation log. Type objectives at the bottom input line. |
| **Graph pane** (right 25%) | Character-cell wiki-graph canvas. Nodes are color-coded by category. |

### Slash Commands

| Command | Effect |
| --- | --- |
| `/model <name>` | Switch model (add `--save` to persist) |
| `/model` or `/model list` | List available models |
| `/reasoning <off\|low\|medium\|high>` | Set reasoning effort |
| `/status` | Show current model, effort, and session info |
| `/clear` | Clear the chat log |
| `/help` | Show available commands |
| `/quit` or `Ctrl+C` | Exit |

### Headless Mode

Run without the TUI for scripting or CI:

```bash
redshank run --no-tui "Investigate ..."
```

## CLI Reference

```bash
redshank [OPTIONS] <COMMAND>

Commands:
  run        Run an investigation with a given objective
  tui        Launch the interactive TUI
  fetch      Run a supported data fetcher directly
  session    List, resume, or delete sessions
  configure  Interactive credential setup
  setup      Alias for configure
  version    Print version and build info

Global Options:
  -w, --workspace <PATH>    Workspace directory [default: .]
  -m, --model <MODEL>       Override the default model
  -r, --reasoning <LEVEL>   Reasoning effort (off|low|medium|high)
      --no-tui              Run headless (no interactive UI)
      --max-depth <N>       Maximum recursion depth for subtasks
      --demo                Demo mode (use mock model)
```

`redshank fetch` currently supports `uk_corporate_intelligence` and its kebab-case alias `uk-corporate-intelligence`.

## Development

### Prerequisites

- Rust 1.94+ (stable)
- SQLite (bundled via rusqlite)

### Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

### Optional Features

```bash
# Build with stygian web automation
cargo build --workspace --features redshank-fetchers/stygian

# Build with Coraline MCP code navigation tools
cargo build -p redshank-core --features coraline
```

### Stygian Setup (JS-heavy sources)

Several fetchers — state Secretary of State portals, county property systems,
and social media profiles — require a real browser to extract content. These
route through [stygian-mcp](https://github.com/stygian-labs/stygian) when it is
running, and **fail-soft** (return empty results with a warning) when it is not.

1. Install stygian-mcp:

   ```bash
   cargo install stygian-mcp --locked
   ```

2. Start it on the default port:

   ```bash
   stygian-mcp --port 8787
   ```

3. Build redshank with the feature enabled and confirm the TUI footer shows
   `stygian: ▲`.

The health probe hits `http://127.0.0.1:8787/health` by default. Run stygian-mcp
on that host and port. Configuration of the probe endpoint via `settings.json`
is planned for a future release.

See the [Stygian Fallback architecture doc](docs/src/architecture/stygian-fallback.md)
for full setup, troubleshooting, and licensing-boundary rationale.

### Workspace Layout

```text
redshank/
├── redshank-core/       Core library: domain model, ports, engine, tools, persistence
│   └── src/
│       ├── domain/      Pure types, zero I/O deps
│       ├── ports/       Trait interfaces (inbound + outbound)
│       ├── application/ CQRS command/query handlers, engine service
│       └── adapters/    LLM providers, tools, SQLite, wiki filesystem
├── redshank-tui/        ratatui terminal interface
├── redshank-fetchers/   34 data-source fetcher libraries
├── redshank-cli/        clap CLI entry point
└── plan.toml            Build plan (26 tasks)
```

## Comparison with OpenPlanter

| | OpenPlanter | Redshank |
| --- | --- | --- |
| **Language** | Python 3.12 | Rust 1.94 (edition 2024) |
| **TUI framework** | Textual | ratatui + crossterm |
| **Graph library** | NetworkX | petgraph |
| **HTTP client** | urllib / httpx | reqwest + stygian-browser |
| **LLM providers** | Anthropic, OpenAI, OpenRouter, Cerebras, Ollama | Same set, native Rust clients |
| **Data fetchers** | 12 Python scripts | 34 library modules |
| **Architecture** | Flat modules | Hexagonal DDD + CQRS |
| **Security model** | File permissions | Typed AuthContext + SecurityPolicy + fail-secure |
| **Session storage** | JSON files | SQLite with domain events |
| **Distribution** | pip install + Python runtime | Single compiled binary |
| **Desktop app** | Tauri 2 + Svelte | — (TUI only, desktop planned) |

## License

See [LICENSE](LICENSE) for details.
