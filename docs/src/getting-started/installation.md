# Installation

## From crates.io

```bash
cargo install redshank-cli --locked
```

## From source

Requires Rust 1.94+ stable.

```bash
git clone https://github.com/greysquirr3l/redshank.git
cd redshank
cargo build --release
```

The binary lands at `target/release/redshank`.

## Optional features

| Feature | Description |
|---------|-------------|
| `stygian` | Enables stygian-graph pipelines and stygian-browser anti-detection for JS-rendered pages. Requires Chrome. |
| `coraline` | Adds Coraline MCP tool bindings for self-directed code navigation. |

Build with a feature:

```bash
cargo build --release --features stygian
```
