//! Social Profile Scraping — Public profile extraction pipeline.
//!
//! Targets: `LinkedIn` (public), Twitter/X, Mastodon (`ActivityPub` JSON).
//! Uses stygian-browser Advanced stealth + `ai_extract` for JS-rendered pages.
//! Only public, search-engine-indexable pages are accessed.
//! Rate: one profile per 5 seconds minimum.
//!
//! Pipeline config stored in `pipelines/social_profiles/config.toml`.

use crate::domain::{FetchError, FetchOutput};
use crate::fallback::{FetchExecutionMode, StygianAvailability, select_execution_mode};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// Pipeline config loaded at compile time.
pub const PIPELINE_CONFIG: &str = include_str!("../../pipelines/social_profiles/config.toml");

/// Minimum delay between profile fetches (5 seconds).
const PROFILE_DELAY_MS: u64 = 5000;

/// A target platform from the pipeline config.
#[derive(Debug, Clone)]
pub struct ProfileTarget {
    pub platform: String,
    pub url_template: String,
    pub requires_browser: bool,
    pub extract_fields: Vec<String>,
}

/// Parse the social profiles pipeline config.
#[must_use]
pub fn parse_pipeline_config(toml_str: &str) -> Vec<ProfileTarget> {
    let mut targets = Vec::new();
    let mut current: Option<ProfileTarget> = None;

    for line in toml_str.lines() {
        let line = line.trim();

        if line == "[[target]]" {
            if let Some(t) = current.take() {
                targets.push(t);
            }
            current = Some(ProfileTarget {
                platform: String::new(),
                url_template: String::new(),
                requires_browser: false,
                extract_fields: Vec::new(),
            });
            continue;
        }

        if line.starts_with('#') || line.is_empty() || line.starts_with('[') {
            continue;
        }

        let Some(t) = current.as_mut() else { continue };

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "platform" => value.trim_matches('"').clone_into(&mut t.platform),
                "url_template" => value.trim_matches('"').clone_into(&mut t.url_template),
                "requires_browser" => t.requires_browser = value == "true",
                "extract_fields" => {
                    let inner = value.trim_start_matches('[').trim_end_matches(']');
                    t.extract_fields = inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    if let Some(t) = current {
        targets.push(t);
    }

    targets
}

/// Fetch a Mastodon profile via `ActivityPub` JSON (no browser needed).
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_mastodon_profile(
    instance: &str,
    username: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    // Enforce rate limit
    tokio::time::sleep(std::time::Duration::from_millis(PROFILE_DELAY_MS)).await;

    let resp = client
        .get(format!("https://{instance}/@{username}"))
        .header("Accept", "application/activity+json")
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let records = vec![json];

    let output_path = output_dir.join("social_mastodon.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "social_profiles".into(),
        attribution: None,
    })
}

/// Select the fetch execution mode for a social profile target.
///
/// Platforms where `requires_browser` is `true` (`LinkedIn`, `Twitter/X`) are
/// routed through stygian when available. Mastodon (`ActivityPub` JSON) is
/// always native HTTP.
#[must_use]
pub const fn execution_mode_for_profile(
    requires_browser: bool,
    availability: &StygianAvailability,
) -> FetchExecutionMode {
    select_execution_mode(requires_browser, availability)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use crate::fallback::{FetchExecutionMode, StygianAvailability};

    #[test]
    fn social_profiles_pipeline_config_validates() {
        let targets = parse_pipeline_config(PIPELINE_CONFIG);
        assert_eq!(targets.len(), 3);

        let linkedin = &targets[0];
        assert_eq!(linkedin.platform, "LinkedIn");
        assert!(linkedin.requires_browser);
        assert!(linkedin.extract_fields.contains(&"headline".to_string()));

        let mastodon = &targets[2];
        assert_eq!(mastodon.platform, "Mastodon");
        assert!(!mastodon.requires_browser);
    }

    #[test]
    fn social_profiles_js_targets_route_to_fallback_when_available() {
        let availability = StygianAvailability::Available {
            endpoint_url: "http://127.0.0.1:8787/health".into(),
        };
        // LinkedIn requires_browser = true → StygianMcpFallback
        assert_eq!(
            execution_mode_for_profile(true, &availability),
            FetchExecutionMode::StygianMcpFallback
        );
    }

    #[test]
    fn social_profiles_native_targets_always_use_native_http() {
        let availability = StygianAvailability::Available {
            endpoint_url: "http://127.0.0.1:8787/health".into(),
        };
        // Mastodon requires_browser = false → NativeHttp even when stygian is available
        assert_eq!(
            execution_mode_for_profile(false, &availability),
            FetchExecutionMode::NativeHttp
        );
    }

    #[test]
    fn social_profiles_js_targets_fail_soft_when_stygian_unavailable() {
        let availability = StygianAvailability::Unavailable(
            crate::fallback::StygianUnavailableReason::FeatureDisabled,
        );
        assert_eq!(
            execution_mode_for_profile(true, &availability),
            FetchExecutionMode::FailSoft
        );
    }
}
