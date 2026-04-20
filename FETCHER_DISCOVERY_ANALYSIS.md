# Fetcher Discovery & Registration Analysis

## Summary

Fetchers are discovered and exposed through **three coordinated layers**:

1. **Source Catalog** (authoritative registry of metadata)
2. **Known Fetchers List** (validation constant)  
3. **Configuration/Settings** (runtime enable/disable)

The CLI and TUI both pull from the same catalog source, but with different mechanisms.

---

## Part 1: CLI Fetcher Discovery

### Current State: **INCOMPLETE**

The CLI's `fetch` subcommand is **only partially wired**.

### File Paths

| File | Lines | Purpose |
|------|-------|---------|
| [redshank-cli/src/main.rs](redshank-cli/src/main.rs#L130-L160) | 130–160 | `FetchSource` enum — defines valid fetcher names |
| [redshank-cli/src/main.rs](redshank-cli/src/main.rs#L680-L730) | 680–730 | `cmd_fetch()` dispatcher — routes to fetcher implementations |
| [redshank-cli/src/setup.rs](redshank-cli/src/setup.rs#L1-50) | 1–50 | Credential setup wizard — credential field registry |

### How CLI Discovers Fetchers

#### 1. **FetchSource Enum** (Static Parser Definition)

```rust
// Line 158 in main.rs
#[derive(Clone, Copy, Debug, ValueEnum)]
enum FetchSource {
    #[value(name = "uk_corporate_intelligence", alias = "uk-corporate-intelligence")]
    UkCorporateIntelligence,
}
```

- Only **ONE** variant currently defined
- Clap uses this to auto-validate CLI args: `redshank fetch uk_corporate_intelligence`
- To add new fetchers, must add variants here

#### 2. **cmd_fetch Dispatcher** (Runtime Routing)

```rust
// Lines 680–730 in main.rs
async fn cmd_fetch(
    workspace: &Path,
    source: FetchSource,
    output: Option<&std::path::Path>,
    query: Option<&str>,
) -> anyhow::Result<()> {
    match source {
        FetchSource::UkCorporateIntelligence => {
            // ✅ Only this branch is implemented
            let result = fetch_uk_corporate_intelligence(...)?;
        }
    }
}
```

- **Current State**: Only handles `UkCorporateIntelligence`
- **Missing**: 30+ fetchers in catalog are advertised but not wired
- No fallback error—would panic if other sources matched

#### 3. **Credential Setup** (NO SOURCE LINKING)

```rust
// Lines 35–300+ in setup.rs
pub const ALL_CREDENTIAL_FIELDS: &[CredentialField] = &[
    CredentialField {
        field_name: "uk_companies_house_api_key",
        label: "UK Companies House API key",
        group: "Corporate Registries",
        ...
    },
    // 30+ other credential fields
];
```

- ⚠️ Credential fields are **NOT linked to specific fetchers**
- Fields correspond to fetcher needs but there's no programmatic link
- Credential setup is **manual/group-based**, not source-specific

---

## Part 2: TUI Fetcher Discovery

### Current State: **FULLY AUTOMATIC**

The TUI displays all sources dynamically from the catalog.

### File Paths

| File | Lines | Purpose |
|------|-------|---------|
| [redshank-tui/src/renderer.rs](redshank-tui/src/renderer.rs#L300-310) | 300–310 | Workbench "Sources" tab renderer |
| [redshank-tui/src/renderer.rs](redshank-tui/src/renderer.rs#L432-550) | 432–550 | `render_source_list()` and `render_source_detail()` panels |

### How TUI Discovers Fetchers

#### 1. **Dynamic Call to all_sources()**

```rust
// Line 304 in renderer.rs
WorkbenchTab::Sources => {
    let sources = all_sources(false);  // ← Dynamic discovery
    render_source_list(frame, panes[0], state, &sources);
    render_source_detail(frame, panes[1], state, &sources);
}
```

#### 2. **Catalog Lookup**

- Calls `all_sources(false)` which returns ALL registered `SourceDescriptor` entries
- Sorted by category, then by title
- No static enum—all sources are data-driven

#### 3. **Credential Field Link**

```rust
// Lines 496–508 in renderer.rs
let credential_label = s.credential_field.unwrap_or("(none)");
// ...
"Credential:   ", Style::default().fg(Color::DarkGray)
// Displays the credential_field from the SourceDescriptor
```

- Displays `credential_field: Option<&'static str>` directly from catalog
- Shows "[set via credentials.json — never entered here]"
- **No validation** that credential field exists in settings

---

## Part 3: Source Catalog (Authoritative Registry)

### File: [redshank-core/src/domain/source_catalog.rs](redshank-core/src/domain/source_catalog.rs)

### Key Functions

| Function | Return Type | Usage |
|----------|-------------|-------|
| `all_sources(enabled_only: bool)` | `Vec<&'static SourceDescriptor>` | **TUI discovery** — returns all or enabled-only |
| `source_by_id(id: &str)` | `Option<&'static SourceDescriptor>` | Config queries — lookup by ID |
| `sources_by_category(cat)` | `Vec<&'static SourceDescriptor>` | Category filtering (not used by CLI/TUI) |
| `all_source_ids()` | `Vec<&'static str>` | ID enumeration (not used by CLI/TUI) |

### the SOURCES Constant (Lines 128–1300+)

```rust
pub static SOURCES: &[SourceDescriptor] = &[
    // ✅ All 100+ fetchers registered here as SourceDescriptor
    
    SourceDescriptor {
        id: "gitlab_profile",
        title: "GitLab Profiles",
        description: "GitLab public user profiles...",
        category: SourceCategory::Osint,
        homepage_url: "https://docs.gitlab.com/ee/api/users.html",
        auth_requirement: AuthRequirement::Optional,
        credential_field: None,        // ← Links to credential field
        enabled_by_default: false,
        access_instructions: "Public API available...",
    },
    // ... 100+ more entries
];
```

### Registration Point #1: Catalog Entries

**For new fetcher "my_fetcher":**

```rust
SourceDescriptor {
    id: "my_fetcher",  // ← Must match KNOWN_FETCHERS entry
    title: "My Fetcher",
    description: "...",
    category: SourceCategory::Corporate,
    homepage_url: "...",
    auth_requirement: AuthRequirement::Optional,
    credential_field: Some("my_fetcher_api_key"),  // ← If credentials needed
    enabled_by_default: true,
    access_instructions: "...",
}
```

---

## Part 4: KNOWN_FETCHERS List (Validation Constant)

### File: [redshank-core/src/domain/settings.rs](redshank-core/src/domain/settings.rs#L171)

```rust
pub const KNOWN_FETCHERS: &[&str] = &[
    // T19: 15 core
    "bls_qcew", "census_acs", "clinical_trials", "cms_open_payments",
    "epa_echo", "fdic", "fec", "icij_leaks", "ofac_sdn", "osha",
    "propublica_990", "sam_gov", "sec_edgar", "senate_lobbying",
    "usaspending",
    
    // T20: 17 extended
    "amazon_authors", "county_property", "courtlistener",
    // ... 14+ more
    
    // T21: OSINT fetchers (includes new ones)
    "gitlab_profile",           // ✅ NEW
    "github_profile",
    "hibp",
    "linkedin_public",
    "reverse_address_public",   // ✅ NEW
    "reverse_phone_basic",      // ✅ NEW
    "social_profiles",
    "stackexchange_profile",    // ✅ NEW
    "username_enum",
    "uspto",
    "voter_reg",
    "wayback",
    "whois_rdap",
    // ... 40+ more
];
```

### Purpose of KNOWN_FETCHERS

1. **Validation** — Ensures `FetchersConfig` only contains known IDs
2. **Documentation** — Lists all valid source IDs programmatically
3. **Test Enforcement** — Unit test verifies every entry is in catalog

```rust
// Line 1344 in source_catalog.rs
#[test]
fn test_source_by_id_returns_known_fetchers() {
    for fetcher_id in crate::domain::settings::KNOWN_FETCHERS {
        assert!(
            source_by_id(fetcher_id).is_some(),
            "Fetcher {fetcher_id} in KNOWN_FETCHERS but not in catalog"
        );
    }
}
```

**The new fetchers are ALREADY added to KNOWN_FETCHERS:**

- `gitlab_profile` ✅
- `stackexchange_profile` ✅
- `reverse_phone_basic` ✅
- `reverse_address_public` ✅

---

## Part 5: Registration Points Summary

### Registration Point #1: Source Catalog

**File**: [redshank-core/src/domain/source_catalog.rs](redshank-core/src/domain/source_catalog.rs#L128)  
**Required for**: TUI display, metadata lookup, credential field binding  
**Status**: ✅ All 4 new OSINT fetchers already registered  
**To add fetcher**: Add `SourceDescriptor` entry to `SOURCES` array

### Registration Point #2: KNOWN_FETCHERS List

**File**: [redshank-core/src/domain/settings.rs](redshank-core/src/domain/settings.rs#L171)  
**Required for**: Settings validation, programmatic enumeration  
**Status**: ✅ All 4 new OSINT fetchers already listed  
**To add fetcher**: Add `"fetcher_id"` string to `KNOWN_FETCHERS` array

### Registration Point #3: CLI FetchSource Enum

**File**: [redshank-cli/src/main.rs](redshank-cli/src/main.rs#L158)  
**Required for**: CLI argument parsing, `redshank fetch <SOURCE>` dispatch  
**Status**: ❌ **NOT updated for new fetchers** — only `uk_corporate_intelligence`  
**To add fetcher**: Add `ValueEnum` variant to `enum FetchSource`

### Registration Point #4: Credential Setup Wizard

**File**: [redshank-cli/src/setup.rs](redshank-cli/src/setup.rs#L37)  
**Required for**: Interactive credential prompt in `redshank configure`  
**Status**: ⚠️ **Partially manual** — credential fields grouped by category, not linked to fetchers  
**To add fetcher**: If credential needed, add `CredentialField` entry (already done for OSINTs)

### Registration Point #5: CLI cmd_fetch Dispatcher

**File**: [redshank-cli/src/main.rs](redshank-cli/src/main.rs#L680)  
**Required for**: Actual fetcher invocation from CLI  
**Status**: ❌ **Only implements uk_corporate_intelligence**  
**To add fetcher**: Add match arm to `cmd_fetch` with call to fetcher function

### Registration Point #6: Fetcher Settings Config

**File**: User's `.redshank/settings.json`  
**Structure**:

```json
{
  "fetchers": {
    "gitlab_profile": {
      "enabled": true,
      "rate_limit_ms": 1000,
      "max_pages": null
    }
  }
}
```

**Auto-populated?**: Yes — `FetchersConfig` defaults to all `KNOWN_FETCHERS` as enabled

---

## Use Flow Summary

### TUI: `/config` → Data Sources Tab

1. **Render workbench**: Calls `all_sources(false)`
2. **Catalog lookup**: Reads `SOURCES` array (100+ entries)
3. **Display list**: Shows title, enabled status, auth requirement
4. **On selection**: Display detail pane with metadata + credential field name
5. **No registration needed** — all sources auto-discovered ✅

### CLI: `redshank fetch <SOURCE>`

1. **Parse args**: Clap validates source against `FetchSource` enum variants
2. **Route to handler**: `cmd_fetch` matches on `FetchSource` variant
3. **Invoke fetcher**: Calls imported fetcher function (e.g., `fetch_uk_corporate_intelligence`)
4. **Only 1 source wired** — others fail with error ❌

### Credential Setup: `redshank configure`

1. **Load wizard**: Lists groups from `ALL_CREDENTIAL_FIELDS`
2. **Display by group**: "LLM Providers", "Corporate Registries", "OSINT", etc.
3. **User enters values**: Applied via `apply_input` to `CredentialBundle`
4. **Save to disk**: Persisted to `.redshank/credentials.json`
5. **Auto-discovery**: New credential fields and groups added to setup wizard on registration

---

## Current Gaps

### CLI Fetch Incomplete ❌

- **Advertised**: 30+ fetchers available (catalog populated)
- **Actually wired**: 1 fetcher (`uk_corporate_intelligence`)
- **Impact**: `redshank fetch gitlab_profile ...` would error or panic

### FetchSource Enum Not Extensible ❌

- Manual `ValueEnum` variants required for CLI parsing
- Adding 30 new variants would require 30 match arms in `cmd_fetch`
- Better approach: Dynamic dispatch via `source_by_id()` + Settings

### Credential Fields Not Linked to Fetchers ⚠️

- Wizard groups credentials by category, not by fetcher
- No validation that a fetcher's `credential_field` exists in setup
- Works by convention (e.g., `"gitlab_api_key"` must be defined somewhere)

---

## Recommendation: How New Fetchers Are Discovered

For the **4 new OSINT fetchers** (gitlab, stackexchange, reverse_phone, reverse_address):

| Discovery Layer | Status | Action |
|---|---|---|
| **Catalog** | ✅ Done | Already registered in `SOURCES` |
| **KNOWN_FETCHERS** | ✅ Done | Already listed in settings.rs |
| **TUI Display** | ✅ Auto | TUI discovers via `all_sources(false)` |
| **Configuration** | ✅ Auto | `FetchersConfig` maps KNOWN_FETCHERS to settings |
| **CLI Fetch** | ❌ Manual | Would need FetchSource enum variant + cmd_fetch match arm |
| **Credential Setup** | ✅ Auto | New fields auto-added to wizard via ALL_CREDENTIAL_FIELDS |

**Bottom line**: New fetchers automatically appear in:

- TUI Configuration → Data Sources tab ✅
- Credential setup wizard (if credentials needed) ✅
- Settings validation (via KNOWN_FETCHERS) ✅

But **NOT** in CLI `redshank fetch` command without manual wiring ❌
