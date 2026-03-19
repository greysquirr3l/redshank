# Contributing

## Prerequisites

- Rust 1.94+ stable (`rustup toolchain install 1.94`)
- `cargo-deny` (`cargo install cargo-deny`)

## Setup

```bash
git clone https://github.com/greysquirr3l/redshank.git
cd redshank
cargo build --workspace
```

## Workflow

1. Create a branch: `git checkout -b feat/my-feature`
2. Write a failing test first (TDD strategy)
3. Implement until the test passes
4. Run the full suite: `cargo test --workspace`
5. Check lint: `cargo clippy --workspace -- -D warnings`
6. Check deps: `cargo deny check`
7. Commit with a [conventional commit](https://www.conventionalcommits.org/) message
8. Open a pull request

## Commit conventions

| Prefix | Use for |
|--------|---------|
| `feat:` | New features |
| `fix:` | Bug fixes |
| `refactor:` | Refactors without behaviour change |
| `test:` | Test additions or fixes |
| `docs:` | Documentation only |

Focus commit messages on user impact, not file counts or line changes.
