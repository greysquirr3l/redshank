# Wiki Graph

The wiki graph tracks entities and their relationships across all findings written during an investigation.

## Storage

Each entity is a Markdown file in `wiki/`. The master index is `wiki/index.md`. Cross-references between entities use bold entity names (`**Entity Name**`) which the graph parser extracts.

## Graph structure

Redshank uses [`petgraph::DiGraph`](https://docs.rs/petgraph) to represent the entity graph:

- **Nodes** — `WikiEntry` structs (entity name, slug, last-updated timestamp)
- **Edges** — directed relationships extracted from cross-reference mentions

Fuzzy matching (Levenshtein distance) resolves name variants across different data sources (e.g. "ACME CORPORATION" and "Acme Corp" merge into one node).

## Rendering in the TUI

The TUI's right-hand pane renders the graph as a character-cell canvas using a simple force-directed layout. Nodes are entity name labels; edges are drawn with `─`, `│`, `╭`, `╰` box-drawing characters.

## Exporting

```bash
redshank wiki export --format dot > graph.dot
dot -Tsvg graph.dot > graph.svg
```

The DOT export is compatible with Graphviz for publication-quality rendering.
