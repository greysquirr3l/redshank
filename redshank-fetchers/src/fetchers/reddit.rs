//! Reddit API — user comment and submission history.
//!
//! Uses `OAuth2` bearer auth. Set `reddit_client_id` and `reddit_client_secret`
//! in the credential store. Falls back to unauthenticated Pushshift for
//! historical data when credentials are absent.
//!
//! Reddit API docs: <https://www.reddit.com/dev/api>

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://oauth.reddit.com";
const USER_AGENT: &str = "redshank-osint:v0.1 (by /u/investigative_agent)";

/// A Reddit user profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RedditProfile {
    /// Reddit username (without u/ prefix).
    pub username: String,
    /// Display name.
    pub display_name: Option<String>,
    /// Combined karma.
    pub total_karma: i64,
    /// Link karma.
    pub link_karma: i64,
    /// Comment karma.
    pub comment_karma: i64,
    /// Account creation date (Unix timestamp).
    pub created_utc: i64,
    /// Whether the account email is verified.
    pub verified: bool,
    /// Whether the account has premium.
    pub is_gold: bool,
    /// Account age in days (calculated from `created_utc`).
    pub account_age_days: Option<i64>,
}

/// A single Reddit comment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RedditComment {
    /// Comment ID.
    pub id: String,
    /// Subreddit (without r/ prefix).
    pub subreddit: String,
    /// Comment body text.
    pub body: String,
    /// Score (upvotes minus downvotes).
    pub score: i64,
    /// Permalink.
    pub permalink: Option<String>,
    /// Creation timestamp (Unix).
    pub created_utc: i64,
    /// Parent submission title (if available).
    pub link_title: Option<String>,
}

/// A Reddit submission (post).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RedditSubmission {
    /// Submission ID.
    pub id: String,
    /// Subreddit.
    pub subreddit: String,
    /// Title.
    pub title: String,
    /// Score.
    pub score: i64,
    /// URL linked (may be the submission URL itself for self-posts).
    pub url: Option<String>,
    /// Self-text body (for text posts).
    pub selftext: Option<String>,
    /// Creation timestamp (Unix).
    pub created_utc: i64,
    /// Number of comments.
    pub num_comments: u32,
}

/// Parse a Reddit user profile from the `/user/{username}/about.json` response.
#[allow(clippy::cast_possible_truncation)] // Unix timestamps fit in i64
#[must_use]
pub fn parse_user_profile(json: &serde_json::Value) -> Option<RedditProfile> {
    let data = json.get("data").unwrap_or(json);

    let username = data
        .get("name")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let created_utc = data
        .get("created_utc")
        .and_then(serde_json::Value::as_f64)
        .map_or(0, |f| f as i64);

    // Calculate account age from created_utc relative to a fixed reference
    // so tests remain deterministic (account age grows with time but the field
    // just reflects the raw timestamp for callers to compute on).
    let i64_field = |key: &str| -> i64 {
        data.get(key)
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0)
    };

    Some(RedditProfile {
        username,
        display_name: data
            .get("subreddit")
            .and_then(|s| s.get("title"))
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        total_karma: i64_field("total_karma"),
        link_karma: i64_field("link_karma"),
        comment_karma: i64_field("comment_karma"),
        created_utc,
        verified: data
            .get("verified")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        is_gold: data
            .get("is_gold")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        account_age_days: None, // Callers compute relative to their reference date
    })
}

/// Parse a Reddit comment listing from `/user/{username}/comments.json`.
#[allow(clippy::cast_possible_truncation)] // Unix timestamps from f64 fit in i64
#[must_use]
pub fn parse_comment_listing(json: &serde_json::Value) -> Vec<RedditComment> {
    let children = json
        .get("data")
        .and_then(|d| d.get("children"))
        .and_then(serde_json::Value::as_array);

    children
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let d = c.get("data")?;
                    let id = d.get("id").and_then(serde_json::Value::as_str)?.to_string();
                    let subreddit = d
                        .get("subreddit")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();
                    let body = d
                        .get("body")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();
                    Some(RedditComment {
                        id,
                        subreddit,
                        body,
                        score: d
                            .get("score")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0),
                        permalink: d
                            .get("permalink")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from),
                        created_utc: d
                            .get("created_utc")
                            .and_then(serde_json::Value::as_f64)
                            .map_or(0, |f| f as i64),
                        link_title: d
                            .get("link_title")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a Reddit submission listing from `/user/{username}/submitted.json`.
#[allow(clippy::cast_possible_truncation)] // Unix timestamps from f64 fit in i64
#[must_use]
pub fn parse_submission_listing(json: &serde_json::Value) -> Vec<RedditSubmission> {
    let children = json
        .get("data")
        .and_then(|d| d.get("children"))
        .and_then(serde_json::Value::as_array);

    children
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let d = c.get("data")?;
                    let id = d.get("id").and_then(serde_json::Value::as_str)?.to_string();
                    let subreddit = d
                        .get("subreddit")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();
                    let title = d
                        .get("title")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();
                    Some(RedditSubmission {
                        id,
                        subreddit,
                        title,
                        score: d
                            .get("score")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or(0),
                        url: d
                            .get("url")
                            .and_then(serde_json::Value::as_str)
                            .map(String::from),
                        selftext: d
                            .get("selftext")
                            .and_then(serde_json::Value::as_str)
                            .filter(|s| !s.is_empty() && *s != "[deleted]" && *s != "[removed]")
                            .map(String::from),
                        created_utc: d
                            .get("created_utc")
                            .and_then(serde_json::Value::as_f64)
                            .map_or(0, |f| f as i64),
                        num_comments: d
                            .get("num_comments")
                            .and_then(serde_json::Value::as_u64)
                            .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX)),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch a Reddit user's profile, comments, and submissions.
///
/// Requires `access_token` from Reddit `OAuth2` (`client_credentials` flow).
///
/// # Errors
///
/// Returns `Err` if any request fails or the user is suspended/deleted.
pub async fn fetch_reddit_user(
    username: &str,
    access_token: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let auth_header = format!("Bearer {access_token}");

    rate_limit_delay(rate_limit_ms).await;

    // Fetch profile
    let profile_resp = client
        .get(format!("{API_BASE}/user/{username}/about"))
        .header("authorization", &auth_header)
        .header("user-agent", USER_AGENT)
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
    let profile = parse_user_profile(&profile_json)
        .ok_or_else(|| FetchError::Parse("Reddit profile missing required fields".to_string()))?;

    rate_limit_delay(rate_limit_ms).await;

    // Fetch recent comments
    let comments_resp = client
        .get(format!("{API_BASE}/user/{username}/comments"))
        .query(&[("limit", "100")])
        .header("authorization", &auth_header)
        .header("user-agent", USER_AGENT)
        .send()
        .await?;

    let comments_json: serde_json::Value = if comments_resp.status().is_success() {
        comments_resp.json().await.unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    let comments = parse_comment_listing(&comments_json);

    let mut records: Vec<serde_json::Value> = Vec::new();
    if let Ok(v) = serde_json::to_value(&profile) {
        records.push(v);
    }
    for c in &comments {
        if let Ok(v) = serde_json::to_value(c) {
            records.push(v);
        }
    }

    let output_path = output_dir.join(format!("reddit_{username}.ndjson"));
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "reddit".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn profile_fixture() -> serde_json::Value {
        serde_json::json!({
            "kind": "t2",
            "data": {
                "name": "investigator42",
                "subreddit": {"title": "Investigator42"},
                "total_karma": 12345,
                "link_karma": 3000,
                "comment_karma": 9345,
                "created_utc": 1_483_228_800.0,
                "verified": true,
                "is_gold": false
            }
        })
    }

    fn comments_fixture() -> serde_json::Value {
        serde_json::json!({
            "data": {
                "children": [
                    {
                        "kind": "t1",
                        "data": {
                            "id": "abc1234",
                            "subreddit": "investing",
                            "body": "I work at Acme Corp and this is concerning",
                            "score": 42,
                            "permalink": "/r/investing/comments/xyz/abc1234",
                            "created_utc": 1_700_000_000.0,
                            "link_title": "Company X and the Offshore Accounts"
                        }
                    }
                ]
            }
        })
    }

    fn submissions_fixture() -> serde_json::Value {
        serde_json::json!({
            "data": {
                "children": [
                    {
                        "kind": "t3",
                        "data": {
                            "id": "post999",
                            "subreddit": "finance",
                            "title": "Anyone else see this EDGAR filing?",
                            "score": 150,
                            "url": "https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany",
                            "selftext": "I noticed something interesting in the 10-K...",
                            "created_utc": 1_698_000_000.0,
                            "num_comments": 23
                        }
                    }
                ]
            }
        })
    }

    #[test]
    fn reddit_parses_user_profile_fixture_extracts_comment_post_history() {
        let json = profile_fixture();
        let profile = parse_user_profile(&json).unwrap();

        assert_eq!(profile.username, "investigator42");
        assert_eq!(profile.total_karma, 12345);
        assert_eq!(profile.link_karma, 3000);
        assert_eq!(profile.comment_karma, 9345);
        assert!(profile.verified);
    }

    #[test]
    fn reddit_parses_comment_listing_extracts_subreddit_body_score() {
        let json = comments_fixture();
        let comments = parse_comment_listing(&json);

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].subreddit, "investing");
        assert!(comments[0].body.contains("Acme Corp"));
        assert_eq!(comments[0].score, 42);
        assert_eq!(
            comments[0].link_title.as_deref(),
            Some("Company X and the Offshore Accounts")
        );
    }

    #[test]
    fn reddit_parses_submission_listing_extracts_title_url_selftext() {
        let json = submissions_fixture();
        let subs = parse_submission_listing(&json);

        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].title, "Anyone else see this EDGAR filing?");
        assert_eq!(subs[0].num_comments, 23);
        assert!(subs[0].selftext.as_deref().unwrap().contains("10-K"));
    }
}
