# Changelog

All notable changes to Redshank are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.4] - 2026-04-19

### Added

- **Paid-tier reverse phone lookup options** — Two new optional fetchers for subscriber lookups:
  - `reverse_phone_twilio`: Carrier detection and line type via Twilio Lookup API (low cost, ~$0.005/lookup).
  - `reverse_phone_truecaller`: Full subscriber information (name, address, carrier) via TrueCaller API (higher cost, ~$0.10+/lookup).

## [0.2.3] - 2026-04-19

### Added

- **Individual OSINT expansion: public profiles & reverse lookups** — Four new fetchers for free/public OSINT sources:
  - `gitlab_profile`: Search public GitLab profiles via the unauthenticated Users API.
  - `stackexchange_profile`: Search public Stack Overflow profiles via the public Stack Exchange API.
  - `reverse_phone_basic`: Normalize US phone numbers to E.164 format with country-code inference (metadata only; no paid identity API).
  - `reverse_address_public`: Geocode and enrich US addresses via the Census geocoder.
- **CLI dynamic fetcher dispatch** — Replaced hardcoded `FetchSource` enum with a string-based registry (`KNOWN_FETCHERS`). All fetchers now wire through the CLI via `redshank fetch <source-id> --query <query>`.
- **Dependency Review workflow resilience** — Added HTTP pre-check guard (via GitHub SBOM API) to skip dependency review on repos where the dependency graph is disabled, preventing hard failures.

### Changed

- **Individual OSINT module documentation** — Updated title from "8 individual-person OSINT fetchers" to dynamic count; updated catalog entries and command snippets to reflect all new fetchers.
- **CLI error messaging** — Unknown fetcher errors now direct users to the TUI Sources tab or documentation instead of the misleading `redshank configure` command.
- **Response shaping test coverage** — Added unit tests for `gitlab_profile` and `stackexchange_profile` response-shaping helpers to verify JSON mapping.

### Fixed

- **Dependency Review workflow reliability** — Added fallback handling (`|| echo "000"`) to curl network calls to prevent transient failures from hard-failing the entire workflow.
- **Memory optimization in reverse_address_public** — Changed from `.cloned().unwrap_or_default()` to `.map_or(&[], Vec::as_slice)` to avoid unnecessary allocations and satisfy `clippy::map_unwrap_or`.

## [0.2.2] - 2026-04-17

### Added

- **Diffable fetcher abstraction for PoL sidecars** — Added `DiffableFetcher` to `pol_sidecar` to formalize the five-step observation pipeline (hash, lookup, delta classification, observation construction, append).
- **Crates publishing workflow** — Added `.github/workflows/publish.yml` with ordered crates.io publishing (`redshank-core` → `redshank-fetchers`/`redshank-tui` → `redshank-cli`), index propagation wait logic, and idempotent skip-if-published behavior.
- **Dependency review and OSSF scorecard workflows** — Added `.github/workflows/dependency-review.yml` and `.github/workflows/scorecard.yml` to strengthen supply-chain and security posture.

### Changed

- **Release orchestration hardening** — `auto-tag.yml` now tags only after successful CI on `main` (workflow_run), with concurrency guard and race-safe tag push behavior.
- **Release workflow reliability** — `release.yml` now resolves tag+SHA across trigger types, verifies CI success for the release commit before build, and builds with `--locked`.
- **Version bump for publish-ready release** — Workspace and internal dependency versions advanced to `0.2.2`.

### Fixed

- **PoL implementation gap closure** — `blockchain_explorer`, `defi_protocols`, and `exchange_transparency` now implement and use `DiffableFetcher::record_observation` instead of repeating inline observation logic.
- **Clippy/warnings cleanup** — Resolved trailing comma and duration unit lints in CLI and core, plus rustdoc markdown lint fixes for `DeFi`/`PoL` terms.
- **Tracking hygiene** — `PROGRESS.md` removed from repository tracking while remaining locally ignored.

## [0.2.1] - 2026-04-15

### Added

- **Stygian fallback for JS-heavy sources** — `redshank-fetchers` now probes for a live stygian-graph/stygian-browser endpoint at startup. Sources that require JavaScript rendering fall back to the stygian pipeline automatically when available, with capability detection and a configurable `FallbackPolicy`.
- **PoL entity observation timeline** — Political-sidecar fetcher (`pol_sidecar`) now emits a timestamped `ObservationTimeline` for each entity, with per-source analytics and full ingestion support in the PoL pipeline.
- **`rpassword` integration** — `redshank configure` now uses `rpassword::prompt_password` for all fields marked `is_secret: true`, preventing API keys and tokens from echoing to the terminal during interactive setup.
- **`PartialEq` / `Eq` derives on `CredentialBundle`** — Required for change-detection in the configure workflow; also useful for testing.

### Changed

- **No-op save prevention** — `redshank configure` now compares the post-input bundle against the original (loaded from disk). If nothing changed, it prints "No changes — nothing saved." instead of rewriting `credentials.json` with an unchanged bundle.
- **`ollama_base_url` set-detection** — Both the per-field `[set]` indicator and the summary credential counter now use `.as_deref().is_some_and(|s| !s.trim().is_empty())` to reject empty or whitespace-only values that would previously be counted as configured.
- **`no_bundle_field_left_behind` test** — Refactored to derive the expected field list from `ALL_CREDENTIAL_FIELDS` as the canonical source instead of a hard-coded static array that could silently drift.

### Fixed

- **Error handling audit** — JSON serialisation failures in `anthropic.rs` and `openai_compat.rs`, config parse failures in `credential_store.rs` and `settings_store.rs`, and async I/O errors in `browser_fallback.rs` and `stygian.rs` now emit `tracing::warn!` instead of being silently swallowed.
- **`wiki_graph.rs` regex initialisation** — Changed `unreachable!()` to `expect()` with SAFETY documentation, and added scoped `#[allow(clippy::expect_used)]` for compile-time-verified regex literals.
- **README code-block indentation** — Removed stray 4-space indent that caused the `redshank fetch` prose line to render as a Markdown code block.
- **Doc comment accuracy** — `ALL_CREDENTIAL_FIELDS` rustdoc correctly refers to `no_bundle_field_left_behind` as a "module-local unit test" rather than "integration test".

### CI

- **Workflow normalization** — CI, release, auto-tag, CodeQL, and security-audit workflows consolidated and normalised across all matrix targets (ubuntu, macOS, Windows). Windows cross-compilation fixed: `process_group(0)` guarded behind `#[cfg(unix)]`, Unix-only `PermissionsExt` usage scoped appropriately. Private and cross-crate intra-doc links that broke the `Documentation` check replaced with plain text.

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

[Unreleased]: https://github.com/greysquirr3l/redshank/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/greysquirr3l/redshank/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/greysquirr3l/redshank/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/greysquirr3l/redshank/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/greysquirr3l/redshank/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/greysquirr3l/redshank/releases/tag/v0.1.0
