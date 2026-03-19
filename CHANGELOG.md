# Changelog

All notable changes to Redshank are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-03-19

### Added

- **ODbL attribution enforcement for OpenCorporates** — New `Attribution` value type in `FetchOutput` carries licence metadata (text, URL, `min_font_size_px`, SPDX identifier) alongside every fetch result. Report and wiki layers must render the hyperlink before surfacing OpenCorporates data. All other fetchers carry `attribution: None`.
- **`opencorporates::attribution()` helper** — Public function returning the fully populated ODbL-1.0 `Attribution` for use at call sites and in tests.
- **OpenCorporates licence documentation** — `docs/src/data-sources/corporate-registries.md` has a dedicated Licence section covering attribution rules, `rel-canonical` requirement, downstream API obligations, and how `FetchOutput::attribution` propagates through redshank.

## [0.1.0] - 2026-03-19

### Added

- **Multi-prefix credential resolution** — Support `REDSHANK_<KEY>`, `OPENPLANTER_<KEY>` (legacy compat), and bare `<KEY>` env vars with three-level priority. Allows running multiple agents on the same host without conflicting credentials.
- **FEC and OpenCorporates credentials** — `FEC_API_KEY` and `OPENCORPORATES_API_KEY` added to `CredentialBundle` for campaign finance and corporate registry fetchers.
- **Example configuration files** — `credentials.example.json`, `settings.example.json`, `.env.example` with all credential and settings fields documented.
- **Gitleaks secret scanning** — Pre-commit hook (`gitleaks protect --staged`) and GitHub Actions workflow (`gitleaks-action@v2`) to catch secrets before they're staged or merged.
- **Release automation** — Cross-platform release workflow (macOS arm64/x86_64, Windows x64) triggered on CI pass + v-tags; auto-generates release notes via `softprops/action-gh-release`.
- **Security audit CI** — Cargo audit via `rustsec/audit-check@v2` on Cargo changes, weekly schedule, and on-demand via workflow dispatch.
- **mdBook documentation site** — Full architecture, usage, and data-source guides with deploy to GitHub Pages.

### Changed

- **Credential storage** — From `~/.config/redshank/credentials.toml` (stale/incorrect) to `<workspace>/.redshank/credentials.json` and `~/.redshank/credentials.json` (persistent user/workspace layer).
- **Settings storage** — From `~/.config/redshank/settings.toml` to `<workspace>/.redshank/settings.json` (single workspace-level file).
- **Configuration resolution order** — Clear four-level merge: env vars → `.env` file → workspace credentials.json → user credentials.json.
- **README configuration section** — Rewritten with accurate paths, JSON examples, and resolution order.

### Fixed

- **mdBook logo path** — Moved logo into `docs/src/assets/img/` (mdBook only serves files inside src tree) and corrected docs image reference.
- **Broken credential file references** — Updated quickstart.md and security.md to reference `.json` extension.
- **Pre-commit hook repository** — Moved from `.git/hooks/` to `.githooks/` (committed, version-controlled, wired via `git config core.hooksPath`).

### Security

- **Secret scanning in CI** — Gitleaks scans all file changes before merge; blocks at pre-commit stage locally.
- **Credential storage permissions** — All `.json` files written `chmod 600`; keys never logged at any level.
- **Role-based access control** — Typed `AuthContext` and `SecurityPolicy` enforced at every data-access path.

[Unreleased]: https://github.com/greysquirr3l/redshank/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/greysquirr3l/redshank/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/greysquirr3l/redshank/releases/tag/v0.1.0
