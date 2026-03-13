# T16 — WikiGraphModel: index parsing, cross-ref extraction, petgraph DAG

> **Depends on**: T-domain-types.

## Goal

Port agent/wiki_graph.py to Rust using petgraph. Parse wiki/index.md,
read individual entry files to extract bold cross-references, fuzzy-match
entity names across the registry, and build a petgraph::DiGraph.


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

- Test: parse_index with a 3-category fixture produces correct (category, title, path) triples.
- Test: bold cross-ref extraction finds '**Acme Corp**' in a fixture entry file.
- Test: fuzzy match finds 'Acme Corporation' node when query is 'Acme Corp' (jaro_winkler > 0.88).
- Test: acronym key 'AC' resolves to 'Acme Corp' node.
- Test: rebuild() after a file write updates the graph node count.


### 2. GREEN — Implement to pass

- WikiGraphModel { graph: DiGraph<WikiNode, WikiEdge>, name_registry: HashMap<String, NodeIndex> }.
- parse_index(index_path) → Vec<(WikiCategory, String, PathBuf)>: parse '## Category' sections, then '- [title](path)' entries.
- For each entry file, extract bold text in '## Cross-Reference Potential' section: **Entity Name** patterns.
- Name registry keys: full name, parentheticals (content inside parens), slash-split parts, acronyms (first letters of each word), slug (lowercase-hyphenated).
- fuzzy_match(name, registry) → Option<NodeIndex>: try exact key lookup; if miss, use strsim::jaro_winkler with threshold 0.88.
- WikiWatcher: use notify crate to watch the wiki dir; on Change event, call rebuild().
- WikiCategory colours (for TUI rendering): CampaignFinance=Cyan, Contracts=Yellow, Corporate=Green, Financial=Red, International=Magenta, Lobbying=Blue, Nonprofits=White, Other=Gray.


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

Update PROGRESS.md row for T16 to `[x]`.
Commit: `feat(wiki-graph): implement wikigraphmodel: index parsing, cross-ref extraction, petgraph dag`
