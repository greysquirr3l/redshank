//! Bluesky social network — actor profiles and feeds via AT Protocol.
//!
//! Source: <https://bsky.social/xrpc/>
//! Public API — no auth required for public profiles.
//! AT Protocol is federated; handles may be on different PDS servers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const BSKY_API: &str = "https://bsky.social/xrpc";

/// A Bluesky actor (user) profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlueskyProfile {
    /// Full handle (e.g. "alice.bsky.social").
    pub handle: String,
    /// DID (Decentralized Identifier).
    pub did: Option<String>,
    /// Display name.
    pub display_name: Option<String>,
    /// Bio / description.
    pub description: Option<String>,
    /// Follower count.
    pub followers_count: Option<u32>,
    /// Following count.
    pub follows_count: Option<u32>,
    /// Post count.
    pub posts_count: Option<u32>,
    /// Account created at (ISO 8601).
    pub created_at: Option<String>,
}

/// A single Bluesky post (feed view item).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlueskyPost {
    /// AT URI of the post.
    pub uri: String,
    /// CID of the post.
    pub cid: Option<String>,
    /// Author handle.
    pub author_handle: Option<String>,
    /// Post text.
    pub text: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: Option<String>,
    /// Like count.
    pub like_count: Option<u32>,
    /// Repost count.
    pub repost_count: Option<u32>,
    /// Reply count.
    pub reply_count: Option<u32>,
}

/// Parse a Bluesky actor profile from `app.bsky.actor.getProfile` response.
#[must_use]
pub fn parse_bsky_profile(json: &serde_json::Value) -> Option<BlueskyProfile> {
    let handle = json
        .get("handle")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    Some(BlueskyProfile {
        handle,
        did: json
            .get("did")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        display_name: json
            .get("displayName")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        description: json
            .get("description")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        followers_count: json
            .get("followersCount")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        follows_count: json
            .get("followsCount")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        posts_count: json
            .get("postsCount")
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        created_at: json
            .get("createdAt")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

/// Parse a Bluesky feed from `app.bsky.feed.getAuthorFeed` response.
#[must_use]
pub fn parse_bsky_feed(json: &serde_json::Value) -> Vec<BlueskyPost> {
    json.get("feed")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let post_obj = item.get("post")?;
                    let uri = post_obj
                        .get("uri")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();

                    // Text lives in post.record.text
                    let text = post_obj
                        .get("record")
                        .and_then(|r| r.get("text"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string();

                    let created_at = post_obj
                        .get("record")
                        .and_then(|r| r.get("createdAt"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from);

                    let author_handle = post_obj
                        .get("author")
                        .and_then(|a| a.get("handle"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from);

                    Some(BlueskyPost {
                        uri,
                        cid: post_obj
                            .get("cid")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from),
                        author_handle,
                        text,
                        created_at,
                        like_count: post_obj
                            .get("likeCount")
                            .and_then(serde_json::Value::as_u64)
                            .and_then(|n| u32::try_from(n).ok()),
                        repost_count: post_obj
                            .get("repostCount")
                            .and_then(serde_json::Value::as_u64)
                            .and_then(|n| u32::try_from(n).ok()),
                        reply_count: post_obj
                            .get("replyCount")
                            .and_then(serde_json::Value::as_u64)
                            .and_then(|n| u32::try_from(n).ok()),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch a Bluesky actor profile and recent feed.
///
/// # Errors
///
/// Returns `Err` if the actor does not exist or the request fails.
pub async fn fetch_bsky_actor(
    handle: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    // Fetch profile
    let profile_resp = client
        .get(format!("{BSKY_API}/app.bsky.actor.getProfile"))
        .query(&[("actor", handle)])
        .send()
        .await?;

    let status = profile_resp.status();
    if !status.is_success() {
        let body = profile_resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let profile_json: serde_json::Value = profile_resp.json().await?;
    let profile = parse_bsky_profile(&profile_json)
        .ok_or_else(|| FetchError::Parse("Bluesky profile missing handle field".to_string()))?;

    rate_limit_delay(rate_limit_ms).await;

    // Fetch feed
    let feed_resp = client
        .get(format!("{BSKY_API}/app.bsky.feed.getAuthorFeed"))
        .query(&[("actor", handle), ("limit", "50")])
        .send()
        .await?;

    let posts = if feed_resp.status().is_success() {
        let feed_json: serde_json::Value = feed_resp
            .json()
            .await
            .unwrap_or_else(|_| serde_json::json!({}));
        parse_bsky_feed(&feed_json)
    } else {
        Vec::new()
    };

    let mut records: Vec<serde_json::Value> = Vec::new();
    if let Ok(v) = serde_json::to_value(&profile) {
        records.push(v);
    }
    for p in &posts {
        if let Ok(v) = serde_json::to_value(p) {
            records.push(v);
        }
    }

    let output_path = output_dir.join(format!("bluesky_{}.ndjson", handle.replace('.', "_")));
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "bluesky".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn profile_fixture() -> serde_json::Value {
        serde_json::json!({
            "did": "did:plc:abc123xyz456",
            "handle": "investigator.bsky.social",
            "displayName": "The Investigator",
            "description": "Journalist covering financial crime and corporate fraud",
            "followersCount": 8200,
            "followsCount": 512,
            "postsCount": 1450,
            "createdAt": "2023-05-01T10:00:00.000Z"
        })
    }

    fn feed_fixture() -> serde_json::Value {
        serde_json::json!({
            "feed": [
                {
                    "post": {
                        "uri": "at://did:plc:abc123xyz456/app.bsky.feed.post/3jxyz",
                        "cid": "bafyreiXXXXXX",
                        "author": {"handle": "investigator.bsky.social", "displayName": "The Investigator"},
                        "record": {
                            "text": "Breaking: leaked documents show company X routed $50M through BVI subsidiary",
                            "createdAt": "2024-11-15T14:30:00.000Z"
                        },
                        "likeCount": 340,
                        "repostCount": 128,
                        "replyCount": 22
                    }
                }
            ]
        })
    }

    #[test]
    fn bluesky_parses_actor_profile_and_feed_via_at_protocol() {
        let json = profile_fixture();
        let profile = parse_bsky_profile(&json).unwrap();

        assert_eq!(profile.handle, "investigator.bsky.social");
        assert_eq!(profile.did.as_deref(), Some("did:plc:abc123xyz456"));
        assert_eq!(profile.followers_count, Some(8200));
        assert_eq!(profile.posts_count, Some(1450));
        assert!(
            profile
                .description
                .as_deref()
                .unwrap()
                .contains("financial crime")
        );
    }

    #[test]
    fn bluesky_parses_feed_post_text_and_engagement_counts() {
        let json = feed_fixture();
        let posts = parse_bsky_feed(&json);

        assert_eq!(posts.len(), 1);
        assert!(posts[0].text.contains("BVI subsidiary"));
        assert_eq!(posts[0].like_count, Some(340));
        assert_eq!(posts[0].repost_count, Some(128));
        assert_eq!(
            posts[0].author_handle.as_deref(),
            Some("investigator.bsky.social")
        );
    }
}
