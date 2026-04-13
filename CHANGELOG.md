# Changelog

All notable changes to Redshank are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-04-13

### Added

- **Source catalog** — 107+ static `SourceDescriptor` entries covering corporate registries, sanctions lists, courts, media, academic, crypto, nonprofits, regulatory, environmental, OSINT, and government sources. Each entry carries auth-requirement, credential keys, rate-limit hints, and enabled-by-default flags.
- **Configuration CQRS layer** — `UpdateProviderConfiguration` and `UpdateSourceConfiguration` commands with idempotency-key enforcement; `GetConfiguredProviders` and `GetConfiguredSources` queries. New `WorkspaceConfigPort` trait and `WorkspaceConfigStore` file-backed adapter.
- **Provider-aware defaults** — `ProviderEndpointConfig::default_for(kind)` returns correct protocol+deployment per provider (`OpenAiCompatible → Local`, `Anthropic/OpenAI/Cerebras → Hosted`). `UpdateProviderConfiguration` and `default_deployment_for` now apply these defaults instead of hardcoding `Native+Hosted`.
- **TUI workbench** — Providers tab (list + detail pane, inline config editing with `e` key, credential-status indicator) and Sources tab (list + detail pane with auth-requirement and credential-field display). `all_sources(false)` computed once per frame and passed by slice to avoid duplicate allocation.
- **Regulatory fetchers (T27)** — SEC EDGAR enforcement, CFTC enforcement, FINRA BrokerCheck (firm), OCC enforcement, FDIC bank data, NCUA credit-union data, FinCEN MSB registry, HHS OIG exclusions, GSA SAM debarment, EPA ECHO enforcement.
- **FARA + FINRA fetchers (T28)** — DOJ FARA registrant/document search, FINRA BrokerCheck full individual search.
- **International registry fetchers (T29)** — UK Companies House, Australian Business Register (ABR), Canadian SEDAR+, OpenCorporates global, World Bank debarment, Interpol notices.
- **Aviation + maritime fetchers (T30)** — FAA N-Number aircraft registry, MarineTraffic/AIS vessel positions.
- **UCC + property fetchers (T31)** — UCC filing search, US property/deed record search.
- **Academic + media fetchers (T32)** — CrossRef DOI metadata, GDELT media intelligence.
- **SEC XBRL fetcher (T33)** — SEC XBRL financial-statement data (EDGAR inline XBRL viewer API).
- **Offshore leaks fetcher (T34)** — ICIJ Offshore Leaks database entity/relationship search.
- **Extended social fetchers (T35)** — LinkedIn public profile, Twitter/X profile, Instagram public, Mastodon profile, Reddit user history.
- **Healthcare fetchers (T36)** — NPI registry physician/facility lookup, CMS Medicare provider data.
- **Business + legal fetchers (T37)** — Dun & Bradstreet DUNS lookup, BvD Orbis, LexisNexis public-records stub.
- **Nonprofit fetchers (T38)** — IRS Tax-Exempt Organization Search, ProPublica Nonprofit Explorer, Candid GuideStar.
- **Crypto fetchers (T39)** — Chainalysis wallet screening, Elliptic wallet screening, CipherTrace AML.
- **Environmental fetchers (T40)** — EPA TRI toxic release inventory, EPA Superfund/CERCLIS site search.
- **EU register fetchers (T41)** — VIES VAT validation, EU Transparency Register, EBA credit-institution register, ESMA FIRDS instrument search.  
- **UK corporate intelligence** — Enriched Companies House fetch wired into CLI (`fetch uk-corporate`).

### Changed

- **Anthropic list-models URL** — Normalised to always include `/v1` path segment when building the models endpoint, even when `base_url` is set without the suffix.
- **FEC auth requirement** — Changed from `AuthRequirement::None` to `AuthRequirement::Optional` (key improves rate limits but is not required). PACER `credential_field` corrected from `"pacer_credentials"` to `"pacer_username"` (matches `CredentialBundle::has_field`).
- **`settings.example.json` / README** — Provider map key renamed `"Ollama"` → `"OpenAiCompatible"` to match the `ProviderKind` serde variant.
- **Source list sort** — Replaced `format!("{:?}", category)` string allocation per comparison with `display_name()` static-str comparison — fully allocation-free.

### Fixed

- Resolved all `clippy::pedantic` warnings across the workspace (indexing-slicing, get-first, match-same-arms, missing-const-for-fn, and others).
- Updated transitive `rustls` dependency to close upstream CVEs.

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

[Unreleased]: https://github.com/greysquirr3l/redshank/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/greysquirr3l/redshank/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/greysquirr3l/redshank/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/greysquirr3l/redshank/releases/tag/v0.1.0
