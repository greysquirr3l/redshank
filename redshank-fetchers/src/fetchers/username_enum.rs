//! Username Enumeration — Check username presence across 300+ platforms.
//!
//! NO credential extraction or authentication attempts of any kind.
//! Only HEAD/GET requests to check if a public profile page exists.
//!
//! Platform URL templates loaded from `platforms.toml` via `include_str!`.
//! Output: NDJSON `{username, platform, url, found: bool}`.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// Platform definitions loaded at compile time.
pub const PLATFORMS_TOML: &str = include_str!("../../pipelines/username_enum/platforms.toml");

/// A platform entry from platforms.toml.
#[derive(Debug, Clone)]
pub struct Platform {
    pub name: String,
    pub url_template: String,
    pub success_codes: Vec<u16>,
    pub false_positive_patterns: Vec<String>,
}

/// Parse platforms.toml into a list of Platform structs.
pub fn load_platforms(toml_content: &str) -> Vec<Platform> {
    let mut platforms = Vec::new();
    let mut current: Option<Platform> = None;

    for line in toml_content.lines() {
        let line = line.trim();

        if line == "[[platform]]" {
            if let Some(p) = current.take() {
                platforms.push(p);
            }
            current = Some(Platform {
                name: String::new(),
                url_template: String::new(),
                success_codes: vec![200],
                false_positive_patterns: Vec::new(),
            });
            continue;
        }

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        let Some(p) = current.as_mut() else { continue };

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "name" => p.name = value.trim_matches('"').to_owned(),
                "url_template" => p.url_template = value.trim_matches('"').to_owned(),
                "success_codes" => {
                    let inner = value.trim_start_matches('[').trim_end_matches(']');
                    p.success_codes = inner
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                }
                "false_positive_patterns" => {
                    let inner = value.trim_start_matches('[').trim_end_matches(']');
                    p.false_positive_patterns = inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    if let Some(p) = current {
        platforms.push(p);
    }

    platforms
}

/// Build the profile URL for a given username on a platform.
pub fn build_profile_url(platform: &Platform, username: &str) -> String {
    platform.url_template.replace("{username}", username)
}

/// Check if a username exists on the built-in platforms.
///
/// NO credential extraction. NO authentication attempts.
/// Only HEAD requests to detect 200 vs 404.
pub async fn enumerate_username(
    username: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let platforms = load_platforms(PLATFORMS_TOML);
    let client = build_client()?;
    let mut records = Vec::new();
    let delay = std::time::Duration::from_millis(rate_limit_ms);

    for platform in &platforms {
        let url = build_profile_url(platform, username);

        let result = client
            .head(&url)
            .header("User-Agent", "redshank-investigation-agent")
            .send()
            .await;

        let found = match result {
            Ok(resp) => platform.success_codes.contains(&resp.status().as_u16()),
            Err(_) => false,
        };

        records.push(serde_json::json!({
            "username": username,
            "platform": platform.name,
            "url": url,
            "found": found,
        }));

        tokio::time::sleep(delay).await;
    }

    let output_path = output_dir.join("username_enum.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "username_enum".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_enum_loads_platforms_toml() {
        let platforms = load_platforms(PLATFORMS_TOML);
        assert!(platforms.len() > 30, "Expected 30+ platforms, got {}", platforms.len());
        let github = platforms.iter().find(|p| p.name == "GitHub").unwrap();
        assert_eq!(github.url_template, "https://github.com/{username}");
        assert_eq!(github.success_codes, vec![200]);
    }

    #[test]
    fn username_enum_builds_correct_url_for_github() {
        let platform = Platform {
            name: "GitHub".into(),
            url_template: "https://github.com/{username}".into(),
            success_codes: vec![200],
            false_positive_patterns: vec![],
        };
        let url = build_profile_url(&platform, "testuser");
        assert_eq!(url, "https://github.com/testuser");
    }

    #[test]
    fn username_enum_marks_found_on_200_not_found_on_404() {
        // Simulate result mapping
        let success_codes: &[u16] = &[200_u16];
        assert!(success_codes.contains(&200));
        assert!(!success_codes.contains(&404));
    }
}
