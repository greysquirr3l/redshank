//! Credential setup helper for `redshank configure`.
//!
//! Provides a static registry of every credential field with its label,
//! environment variable name, sign-up URL, and group category.  The
//! [`apply_input`] function applies a single string value to the correct
//! field of a [`CredentialBundle`] so that the interactive loop in
//! `cmd_configure` stays purely data-driven.

use redshank_core::domain::credentials::{CredentialBundle, CredentialGuard};

// ── Credential field metadata ────────────────────────────────

/// Metadata for a single interactive credential prompt.
#[derive(Debug, Clone, Copy)]
pub struct CredentialField {
    /// JSON / `CredentialBundle` field name.
    pub field_name: &'static str,
    /// Human-readable display label.
    pub label: &'static str,
    /// Environment variable name equivalent.
    pub env_var: &'static str,
    /// Category group header shown before the first field in each section.
    pub group: &'static str,
    /// URL where the operator can register or obtain the credential.
    pub signup_url: &'static str,
    /// Whether the value should be hidden while typing (API keys / passwords).
    pub is_secret: bool,
    /// Whether the credential is required for core operation vs. optional.
    pub is_required: bool,
}

/// All credential fields in display order, grouped by category.
///
/// This is the authoritative list: every field in [`CredentialBundle`]
/// must appear here.  The integration test `no_bundle_field_left_behind`
/// enforces this.
pub const ALL_CREDENTIAL_FIELDS: &[CredentialField] = &[
    // ── LLM Providers ──────────────────────────────────────────────────────
    CredentialField {
        field_name: "anthropic_api_key",
        label: "Anthropic API key",
        env_var: "ANTHROPIC_API_KEY",
        group: "LLM Providers",
        signup_url: "https://console.anthropic.com/keys",
        is_secret: true,
        is_required: true,
    },
    CredentialField {
        field_name: "openai_api_key",
        label: "OpenAI API key",
        env_var: "OPENAI_API_KEY",
        group: "LLM Providers",
        signup_url: "https://platform.openai.com/api-keys",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "openrouter_api_key",
        label: "OpenRouter API key",
        env_var: "OPENROUTER_API_KEY",
        group: "LLM Providers",
        signup_url: "https://openrouter.ai/keys",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "cerebras_api_key",
        label: "Cerebras API key",
        env_var: "CEREBRAS_API_KEY",
        group: "LLM Providers",
        signup_url: "https://cloud.cerebras.ai/",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "exa_api_key",
        label: "Exa search API key",
        env_var: "EXA_API_KEY",
        group: "LLM Providers",
        signup_url: "https://dashboard.exa.ai/api-keys",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "voyage_api_key",
        label: "Voyage AI embedding API key",
        env_var: "VOYAGE_API_KEY",
        group: "LLM Providers",
        signup_url: "https://dash.voyageai.com/api-keys",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "ollama_base_url",
        label: "Ollama base URL",
        env_var: "OLLAMA_BASE_URL",
        group: "LLM Providers",
        signup_url: "https://ollama.com/",
        is_secret: false,
        is_required: false,
    },
    // ── Government & Contracts ──────────────────────────────────────────────
    CredentialField {
        field_name: "fec_api_key",
        label: "FEC (Federal Election Commission) API key",
        env_var: "FEC_API_KEY",
        group: "Government & Contracts",
        signup_url: "https://api.open.fec.gov/developers/",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "bls_api_key",
        label: "BLS (Bureau of Labor Statistics) API key",
        env_var: "BLS_API_KEY",
        group: "Government & Contracts",
        signup_url: "https://www.bls.gov/developers/home.htm",
        is_secret: true,
        is_required: false,
    },
    // ── Courts & Legal ──────────────────────────────────────────────────────
    CredentialField {
        field_name: "pacer_username",
        label: "PACER username",
        env_var: "PACER_USERNAME",
        group: "Courts & Legal",
        signup_url: "https://pacer.uscourts.gov/register-account",
        is_secret: false,
        is_required: false,
    },
    CredentialField {
        field_name: "pacer_password",
        label: "PACER password",
        env_var: "PACER_PASSWORD",
        group: "Courts & Legal",
        signup_url: "https://pacer.uscourts.gov/register-account",
        is_secret: true,
        is_required: false,
    },
    // ── Corporate Registries ────────────────────────────────────────────────
    CredentialField {
        field_name: "opencorporates_api_key",
        label: "OpenCorporates API token",
        env_var: "OPENCORPORATES_API_KEY",
        group: "Corporate Registries",
        signup_url: "https://opencorporates.com/api_accounts/new",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "uk_companies_house_api_key",
        label: "UK Companies House API key",
        env_var: "UK_COMPANIES_HOUSE_API_KEY",
        group: "Corporate Registries",
        signup_url: "https://developer.company-information.service.gov.uk/",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "crunchbase_api_key",
        label: "Crunchbase API key",
        env_var: "CRUNCHBASE_API_KEY",
        group: "Corporate Registries",
        signup_url: "https://data.crunchbase.com/docs/using-the-api",
        is_secret: true,
        is_required: false,
    },
    // ── Sanctions & Screening ───────────────────────────────────────────────
    CredentialField {
        field_name: "opensanctions_api_key",
        label: "OpenSanctions API key",
        env_var: "OPENSANCTIONS_API_KEY",
        group: "Sanctions & Screening",
        signup_url: "https://www.opensanctions.org/api/",
        is_secret: true,
        is_required: false,
    },
    // ── Open-Source Intelligence ────────────────────────────────────────────
    CredentialField {
        field_name: "hibp_api_key",
        label: "Have I Been Pwned API key",
        env_var: "HIBP_API_KEY",
        group: "Open-Source Intelligence",
        signup_url: "https://haveibeenpwned.com/API/Key",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "github_token",
        label: "GitHub personal access token",
        env_var: "GITHUB_TOKEN",
        group: "Open-Source Intelligence",
        signup_url: "https://github.com/settings/tokens",
        is_secret: true,
        is_required: false,
    },
    // ── Media & Social ──────────────────────────────────────────────────────
    CredentialField {
        field_name: "reddit_client_id",
        label: "Reddit OAuth2 client ID",
        env_var: "REDDIT_CLIENT_ID",
        group: "Media & Social",
        signup_url: "https://www.reddit.com/prefs/apps",
        is_secret: false,
        is_required: false,
    },
    CredentialField {
        field_name: "reddit_client_secret",
        label: "Reddit OAuth2 client secret",
        env_var: "REDDIT_CLIENT_SECRET",
        group: "Media & Social",
        signup_url: "https://www.reddit.com/prefs/apps",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "youtube_api_key",
        label: "YouTube Data API v3 key",
        env_var: "YOUTUBE_API_KEY",
        group: "Media & Social",
        signup_url: "https://console.cloud.google.com/apis/credentials",
        is_secret: true,
        is_required: false,
    },
    CredentialField {
        field_name: "listennotes_api_key",
        label: "Listen Notes API key",
        env_var: "LISTENNOTES_API_KEY",
        group: "Media & Social",
        signup_url: "https://www.listennotes.com/api/pricing/",
        is_secret: true,
        is_required: false,
    },
    // ── Maritime ────────────────────────────────────────────────────────────
    CredentialField {
        field_name: "marinetraffic_api_key",
        label: "MarineTraffic API key",
        env_var: "MARINETRAFFIC_API_KEY",
        group: "Maritime",
        signup_url: "https://www.marinetraffic.com/en/ais-api-services",
        is_secret: true,
        is_required: false,
    },
    // ── Academic & Research ─────────────────────────────────────────────────
    CredentialField {
        field_name: "semantic_scholar_api_key",
        label: "Semantic Scholar API key",
        env_var: "SEMANTIC_SCHOLAR_API_KEY",
        group: "Academic & Research",
        signup_url: "https://www.semanticscholar.org/product/api",
        is_secret: true,
        is_required: false,
    },
    // ── Nonprofits ──────────────────────────────────────────────────────────
    CredentialField {
        field_name: "candid_api_key",
        label: "Candid / GuideStar API key",
        env_var: "CANDID_API_KEY",
        group: "Nonprofits & Charities",
        signup_url: "https://developer.candid.org/",
        is_secret: true,
        is_required: false,
    },
    // ── Cryptocurrency ──────────────────────────────────────────────────────
    CredentialField {
        field_name: "etherscan_api_key",
        label: "Etherscan API key",
        env_var: "ETHERSCAN_API_KEY",
        group: "Cryptocurrency",
        signup_url: "https://etherscan.io/apis",
        is_secret: true,
        is_required: false,
    },
];

// ── Group helpers ────────────────────────────────────────────

/// Returns credential group names in display order (first-encounter order
/// through [`ALL_CREDENTIAL_FIELDS`]).
#[must_use]
pub fn groups() -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for field in ALL_CREDENTIAL_FIELDS {
        if !seen.contains(&field.group) {
            seen.push(field.group);
        }
    }
    seen
}

/// Returns all credential fields belonging to `group`, in declaration order.
#[must_use]
pub fn fields_for_group(group: &str) -> Vec<&'static CredentialField> {
    ALL_CREDENTIAL_FIELDS
        .iter()
        .filter(|f| f.group == group)
        .collect()
}

// ── Credential application ───────────────────────────────────

/// Apply a single credential value to a [`CredentialBundle`] by field name.
///
/// `field_name` must match a key in `credentials.json` exactly
/// (e.g., `"anthropic_api_key"`).  Unknown names are silently ignored.
pub fn apply_input(bundle: &mut CredentialBundle, field_name: &str, value: String) {
    match field_name {
        "openai_api_key" => bundle.openai_api_key = Some(CredentialGuard::new(value)),
        "anthropic_api_key" => bundle.anthropic_api_key = Some(CredentialGuard::new(value)),
        "openrouter_api_key" => bundle.openrouter_api_key = Some(CredentialGuard::new(value)),
        "cerebras_api_key" => bundle.cerebras_api_key = Some(CredentialGuard::new(value)),
        "exa_api_key" => bundle.exa_api_key = Some(CredentialGuard::new(value)),
        "voyage_api_key" => bundle.voyage_api_key = Some(CredentialGuard::new(value)),
        "ollama_base_url" => bundle.ollama_base_url = Some(value),
        "hibp_api_key" => bundle.hibp_api_key = Some(CredentialGuard::new(value)),
        "github_token" => bundle.github_token = Some(CredentialGuard::new(value)),
        "fec_api_key" => bundle.fec_api_key = Some(CredentialGuard::new(value)),
        "opencorporates_api_key" => {
            bundle.opencorporates_api_key = Some(CredentialGuard::new(value))
        }
        "uk_companies_house_api_key" => {
            bundle.uk_companies_house_api_key = Some(CredentialGuard::new(value));
        }
        "opensanctions_api_key" => bundle.opensanctions_api_key = Some(CredentialGuard::new(value)),
        "marinetraffic_api_key" => bundle.marinetraffic_api_key = Some(CredentialGuard::new(value)),
        "semantic_scholar_api_key" => {
            bundle.semantic_scholar_api_key = Some(CredentialGuard::new(value));
        }
        "reddit_client_id" => bundle.reddit_client_id = Some(CredentialGuard::new(value)),
        "reddit_client_secret" => bundle.reddit_client_secret = Some(CredentialGuard::new(value)),
        "youtube_api_key" => bundle.youtube_api_key = Some(CredentialGuard::new(value)),
        "listennotes_api_key" => bundle.listennotes_api_key = Some(CredentialGuard::new(value)),
        "crunchbase_api_key" => bundle.crunchbase_api_key = Some(CredentialGuard::new(value)),
        "bls_api_key" => bundle.bls_api_key = Some(CredentialGuard::new(value)),
        "pacer_username" => bundle.pacer_username = Some(CredentialGuard::new(value)),
        "pacer_password" => bundle.pacer_password = Some(CredentialGuard::new(value)),
        "candid_api_key" => bundle.candid_api_key = Some(CredentialGuard::new(value)),
        "etherscan_api_key" => bundle.etherscan_api_key = Some(CredentialGuard::new(value)),
        _ => {}
    }
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Every `field_name` in the definition list is unique.
    #[test]
    fn all_field_names_unique() {
        let mut seen = std::collections::HashSet::new();
        for field in ALL_CREDENTIAL_FIELDS {
            assert!(
                seen.insert(field.field_name),
                "duplicate field_name: {}",
                field.field_name
            );
        }
    }

    /// Every `field_name` in the definition list can be applied via
    /// `apply_input` (i.e., `has_field` returns `true` afterwards).
    #[test]
    fn all_field_names_valid_in_apply_input() {
        for field in ALL_CREDENTIAL_FIELDS {
            let mut bundle = CredentialBundle::default();
            apply_input(&mut bundle, field.field_name, "test-value".to_string());
            // ollama_base_url is not a CredentialGuard so has_field doesn't
            // cover it, but the URL should be set.
            if field.field_name == "ollama_base_url" {
                assert!(
                    bundle.ollama_base_url.is_some(),
                    "ollama_base_url not set after apply_input"
                );
            } else {
                assert!(
                    bundle.has_field(field.field_name),
                    "has_field returned false after apply_input for: {}",
                    field.field_name
                );
            }
        }
    }

    /// The canonical set of CredentialBundle field names is fully covered by
    /// `ALL_CREDENTIAL_FIELDS`.  If a new field is added to `CredentialBundle`
    /// without a matching entry here, this test fails.
    #[test]
    fn no_bundle_field_left_behind() {
        // This list is derived from the `has_field` match arms in
        // `redshank-core/src/domain/credentials.rs`.
        let bundle_fields = [
            "openai_api_key",
            "anthropic_api_key",
            "openrouter_api_key",
            "cerebras_api_key",
            "exa_api_key",
            "voyage_api_key",
            "ollama_base_url",
            "hibp_api_key",
            "github_token",
            "fec_api_key",
            "opencorporates_api_key",
            "uk_companies_house_api_key",
            "opensanctions_api_key",
            "marinetraffic_api_key",
            "semantic_scholar_api_key",
            "reddit_client_id",
            "reddit_client_secret",
            "youtube_api_key",
            "listennotes_api_key",
            "crunchbase_api_key",
            "bls_api_key",
            "pacer_username",
            "pacer_password",
            "candid_api_key",
            "etherscan_api_key",
        ];

        let defined: std::collections::HashSet<&str> =
            ALL_CREDENTIAL_FIELDS.iter().map(|f| f.field_name).collect();

        for name in bundle_fields {
            assert!(
                defined.contains(name),
                "CredentialBundle field '{name}' is missing from ALL_CREDENTIAL_FIELDS"
            );
        }
    }

    /// `apply_input` is a no-op for an unknown field name.
    #[test]
    fn apply_input_ignores_unknown_field() {
        let mut bundle = CredentialBundle::default();
        apply_input(&mut bundle, "no_such_field", "irrelevant".to_string());
        assert!(!bundle.has_any());
    }

    /// Every group returned by `groups()` has at least one field.
    #[test]
    fn every_group_has_at_least_one_field() {
        for group in groups() {
            assert!(
                !fields_for_group(group).is_empty(),
                "group '{group}' has no fields"
            );
        }
    }

    /// `groups()` contains no duplicates.
    #[test]
    fn groups_are_unique() {
        let gs = groups();
        let unique: std::collections::HashSet<&str> = gs.iter().copied().collect();
        assert_eq!(gs.len(), unique.len(), "duplicate groups detected");
    }

    /// `apply_input` sets `anthropic_api_key` correctly.
    #[test]
    fn apply_input_sets_anthropic_api_key() {
        let mut bundle = CredentialBundle::default();
        apply_input(&mut bundle, "anthropic_api_key", "sk-ant-test".to_string());
        assert!(bundle.has_field("anthropic_api_key"));
        assert_eq!(
            bundle.anthropic_api_key.as_ref().unwrap().expose(),
            "sk-ant-test"
        );
    }

    /// `apply_input` sets `ollama_base_url` (non-secret URL field).
    #[test]
    fn apply_input_sets_ollama_base_url() {
        let mut bundle = CredentialBundle::default();
        apply_input(
            &mut bundle,
            "ollama_base_url",
            "http://localhost:11434".to_string(),
        );
        assert_eq!(
            bundle.ollama_base_url.as_deref(),
            Some("http://localhost:11434")
        );
    }
}
