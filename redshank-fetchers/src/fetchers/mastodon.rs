//! Mastodon ActivityPub API — federated social network intelligence.
//!
//! Source: `https://{instance}/api/v1/`
//! Public endpoints require no auth. Instance is parsed from `user@instance` format.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

/// An account on a Mastodon instance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MastodonAccount {
    /// Account numeric ID on this instance.
    pub id: String,
    /// Username without instance (e.g. "alice").
    pub username: String,
    /// Username with instance (e.g. "alice@mastodon.social").
    pub acct: String,
    /// Display name.
    pub display_name: Option<String>,
    /// Bio / note (HTML).
    pub note: Option<String>,
    /// Profile URL.
    pub url: Option<String>,
    /// Follower count.
    pub followers_count: u32,
    /// Following count.
    pub following_count: u32,
    /// Toot / status count.
    pub statuses_count: u32,
    /// Account creation date (ISO 8601).
    pub created_at: Option<String>,
    /// Instance domain (extracted from the acct or URL).
    pub instance: String,
}

/// A single Mastodon status (toot).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MastodonStatus {
    /// Status numeric ID.
    pub id: String,
    /// HTML content of the toot.
    pub content: String,
    /// Post creation timestamp (ISO 8601).
    pub created_at: String,
    /// Favourite count.
    pub favourites_count: u32,
    /// Boost / reblog count.
    pub reblogs_count: u32,
    /// Reply count.
    pub replies_count: Option<u32>,
    /// Canonical URL.
    pub url: Option<String>,
    /// Whether the status has been boosted (reblogged).
    pub reblog: bool,
    /// Visibility: "public", "unlisted", "private", "direct".
    pub visibility: Option<String>,
}

/// Extract instance domain from an `acct@instance` string or canonical URL.
///
/// Returns `None` if neither can be resolved.
#[must_use]
pub fn resolve_instance(acct: &str, fallback_url: Option<&str>) -> Option<String> {
    if let Some(at_pos) = acct.rfind('@') {
        let domain = &acct[at_pos + 1..];
        if !domain.is_empty() {
            return Some(domain.to_string());
        }
    }
    // Fall back to parsing from URL
    fallback_url.and_then(|url| {
        url.trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .filter(|s| !s.is_empty())
            .map(String::from)
    })
}

/// Parse a Mastodon account lookup response.
#[must_use]
pub fn parse_mastodon_account(json: &serde_json::Value, instance: &str) -> Option<MastodonAccount> {
    let id = json.get("id").and_then(serde_json::Value::as_str)?.to_string();
    let username = json
        .get("username")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let acct = json
        .get("acct")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(&username)
        .to_string();

    Some(MastodonAccount {
        id,
        username,
        acct,
        display_name: json
            .get("display_name")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        note: json
            .get("note")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        url: json
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        followers_count: json
            .get("followers_count")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32)
            .unwrap_or(0),
        following_count: json
            .get("following_count")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32)
            .unwrap_or(0),
        statuses_count: json
            .get("statuses_count")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32)
            .unwrap_or(0),
        created_at: json
            .get("created_at")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        instance: instance.to_string(),
    })
}

/// Parse a Mastodon statuses (timeline) response.
#[must_use]
pub fn parse_mastodon_statuses(json: &serde_json::Value) -> Vec<MastodonStatus> {
    json.as_array()
        .map(|arr| arr.iter().filter_map(parse_single_status).collect())
        .unwrap_or_default()
}

fn parse_single_status(item: &serde_json::Value) -> Option<MastodonStatus> {
    let id = item.get("id").and_then(serde_json::Value::as_str)?.to_string();
    let content = item
        .get("content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let created_at = item
        .get("created_at")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let reblog = item.get("reblog").is_some_and(|v| !v.is_null());

    Some(MastodonStatus {
        id,
        content,
        created_at,
        favourites_count: item
            .get("favourites_count")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32)
            .unwrap_or(0),
        reblogs_count: item
            .get("reblogs_count")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32)
            .unwrap_or(0),
        replies_count: item
            .get("replies_count")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        url: item
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        reblog,
        visibility: item
            .get("visibility")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

/// Fetch a Mastodon account profile and recent statuses.
///
/// # Arguments
///
/// * `acct` — The full `user@instance` handle. Instance is extracted automatically.
/// * `output_dir` — Directory for NDJSON output.
/// * `rate_limit_ms` — Minimum delay between requests.
///
/// # Errors
///
/// Returns `Err` if the account is not found or the request fails.
pub async fn fetch_mastodon_account(
    acct: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    // Parse instance from acct
    let instance = resolve_instance(acct, None).ok_or_else(|| {
        FetchError::Parse(format!("cannot resolve Mastodon instance from acct '{acct}'"))
    })?;

    let username = acct
        .split('@')
        .next()
        .ok_or_else(|| FetchError::Parse("acct must be non-empty".to_string()))?;

    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    let lookup_resp = client
        .get(format!("https://{instance}/api/v1/accounts/lookup"))
        .query(&[("acct", username)])
        .send()
        .await?;

    let status = lookup_resp.status();
    if !status.is_success() {
        let body = lookup_resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let account_json: serde_json::Value = lookup_resp.json().await?;
    let account = parse_mastodon_account(&account_json, &instance)
        .ok_or_else(|| FetchError::Parse("Mastodon account missing required fields".to_string()))?;

    rate_limit_delay(rate_limit_ms).await;

    // Fetch recent statuses
    let account_id = account.id.clone();
    let statuses_resp = client
        .get(format!("https://{instance}/api/v1/accounts/{account_id}/statuses"))
        .query(&[("limit", "40"), ("exclude_replies", "false")])
        .send()
        .await?;

    let statuses = if statuses_resp.status().is_success() {
        let statuses_json: serde_json::Value =
            statuses_resp.json().await.unwrap_or(serde_json::json!([]));
        parse_mastodon_statuses(&statuses_json)
    } else {
        Vec::new()
    };

    let mut records: Vec<serde_json::Value> = Vec::new();
    if let Ok(v) = serde_json::to_value(&account) {
        records.push(v);
    }
    for s in &statuses {
        if let Ok(v) = serde_json::to_value(s) {
            records.push(v);
        }
    }

    let safe_name = acct.replace('@', "_at_");
    let output_path = output_dir.join(format!("mastodon_{safe_name}.ndjson"));
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "mastodon".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn account_fixture() -> serde_json::Value {
        serde_json::json!({
            "id": "109876543210",
            "username": "journalist",
            "acct": "journalist@mastodon.social",
            "display_name": "Freelance Journalist",
            "note": "<p>Investigative journalist covering financial crime.</p>",
            "url": "https://mastodon.social/@journalist",
            "followers_count": 12300,
            "following_count": 456,
            "statuses_count": 3200,
            "created_at": "2022-11-08T00:00:00.000Z"
        })
    }

    fn statuses_fixture() -> serde_json::Value {
        serde_json::json!([
            {
                "id": "111222333444",
                "content": "<p>New report: Panama Papers successor found 2,000 new shell company chains</p>",
                "created_at": "2024-10-01T09:00:00.000Z",
                "favourites_count": 89,
                "reblogs_count": 34,
                "replies_count": 12,
                "url": "https://mastodon.social/@journalist/111222333444",
                "visibility": "public",
                "reblog": null
            },
            {
                "id": "111222333555",
                "content": "<p>Boost</p>",
                "created_at": "2024-10-01T08:00:00.000Z",
                "favourites_count": 0,
                "reblogs_count": 0,
                "url": null,
                "visibility": "public",
                "reblog": {"id": "999999999"}
            }
        ])
    }

    #[test]
    fn mastodon_resolves_instance_from_acct_format() {
        assert_eq!(
            resolve_instance("journalist@mastodon.social", None),
            Some("mastodon.social".to_string())
        );
        assert_eq!(
            resolve_instance("user@fosstodon.org", None),
            Some("fosstodon.org".to_string())
        );
        assert_eq!(
            resolve_instance("bare_user", Some("https://infosec.exchange/@bare_user")),
            Some("infosec.exchange".to_string())
        );
    }

    #[test]
    fn mastodon_parses_account_follower_counts_and_bio() {
        let json = account_fixture();
        let account = parse_mastodon_account(&json, "mastodon.social").unwrap();

        assert_eq!(account.id, "109876543210");
        assert_eq!(account.username, "journalist");
        assert_eq!(account.followers_count, 12_300);
        assert_eq!(account.statuses_count, 3200);
        assert_eq!(account.instance, "mastodon.social");
        assert!(account.note.as_deref().unwrap().contains("financial crime"));
    }

    #[test]
    fn mastodon_parses_status_engagement_counts_and_reblog_detection() {
        let json = statuses_fixture();
        let statuses = parse_mastodon_statuses(&json);

        assert_eq!(statuses.len(), 2);
        assert!(statuses[0].content.contains("Panama Papers"));
        assert_eq!(statuses[0].favourites_count, 89);
        assert_eq!(statuses[0].reblogs_count, 34);
        assert_eq!(statuses[0].replies_count, Some(12));
        assert!(!statuses[0].reblog);
        assert!(statuses[1].reblog);
    }
}
