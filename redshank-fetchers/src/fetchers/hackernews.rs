//! Hacker News public API — user profiles and submission history.
//!
//! Source: <https://hacker-news.firebaseio.com/v0/>
//! Public API, no authentication required.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://hacker-news.firebaseio.com/v0";

/// A Hacker News user profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HnProfile {
    /// HN username (case-sensitive).
    pub id: String,
    /// Account creation timestamp (Unix).
    pub created: i64,
    /// Total karma.
    pub karma: i64,
    /// "About" / bio text (HTML markdown).
    pub about: Option<String>,
    /// IDs of items submitted by this user.
    pub submitted_ids: Vec<u64>,
}

/// A single HN item (story, comment, job, poll, etc.).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HnItem {
    /// Item ID.
    pub id: u64,
    /// Item type ("story", "comment", "job", "poll", "pollopt").
    pub item_type: String,
    /// Author.
    pub by: Option<String>,
    /// Score (for stories and polls).
    pub score: Option<i64>,
    /// Title (for stories, jobs, polls).
    pub title: Option<String>,
    /// URL linked (for stories).
    pub url: Option<String>,
    /// Text content (for comments, self posts, jobs).
    pub text: Option<String>,
    /// Timestamp (Unix).
    pub time: i64,
    /// Number of descendants (comments).
    pub descendants: Option<u32>,
}

/// Parse a Hacker News user profile from the Firebase REST API response.
#[must_use]
pub fn parse_hn_profile(json: &serde_json::Value) -> Option<HnProfile> {
    let id = json
        .get("id")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let created = json
        .get("created")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let karma = json
        .get("karma")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);

    let submitted_ids = json
        .get("submitted")
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(serde_json::Value::as_u64).collect())
        .unwrap_or_default();

    Some(HnProfile {
        id,
        created,
        karma,
        about: json
            .get("about")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        submitted_ids,
    })
}

/// Parse a Hacker News item from the Firebase REST API response.
#[must_use]
pub fn parse_hn_item(json: &serde_json::Value) -> Option<HnItem> {
    let id = json.get("id").and_then(serde_json::Value::as_u64)?;
    let item_type = json
        .get("type")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let time = json
        .get("time")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);

    Some(HnItem {
        id,
        item_type,
        by: json
            .get("by")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        score: json.get("score").and_then(serde_json::Value::as_i64),
        title: json
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        url: json
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        text: json
            .get("text")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        time,
        descendants: json
            .get("descendants")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
    })
}

/// Fetch a Hacker News user profile (and optionally recent submissions).
///
/// # Errors
///
/// Returns `Err` if the user is not found or the request fails.
pub async fn fetch_hn_user(
    username: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    let resp = client
        .get(format!("{API_BASE}/user/{username}.json"))
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let json: serde_json::Value = resp.json().await?;

    // Firebase returns `null` for missing users
    if json.is_null() {
        return Err(FetchError::ApiError {
            status: 404,
            body: format!("HN user '{username}' not found"),
        });
    }

    let profile = parse_hn_profile(&json)
        .ok_or_else(|| FetchError::Parse("HN profile missing required fields".to_string()))?;

    let serialized =
        serde_json::to_value(&profile).map_err(|e| FetchError::Parse(e.to_string()))?;

    let output_path = output_dir.join(format!("hn_{username}.ndjson"));
    let count = write_ndjson(&output_path, &[serialized])?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "hackernews".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn profile_fixture() -> serde_json::Value {
        serde_json::json!({
            "id": "pg",
            "created": 1_160_418_092,
            "karma": 155223,
            "about": "This is <a href=\"http://paulgraham.com\">my home page</a>.",
            "submitted": [141865, 141564, 140813, 140190, 139309]
        })
    }

    fn item_fixture() -> serde_json::Value {
        serde_json::json!({
            "id": 8863,
            "type": "story",
            "by": "dhouston",
            "score": 111,
            "title": "My YC app: Dropbox - Throw away your USB drive",
            "url": "http://www.getdropbox.com/u/2/screencast.html",
            "time": 1_175_714_200,
            "descendants": 71
        })
    }

    #[test]
    fn hackernews_parses_user_profile_fixture_extracts_karma_and_submissions() {
        let json = profile_fixture();
        let profile = parse_hn_profile(&json).unwrap();

        assert_eq!(profile.id, "pg");
        assert_eq!(profile.karma, 155_223);
        assert_eq!(profile.submitted_ids.len(), 5);
        assert!(profile.about.as_deref().unwrap().contains("paulgraham.com"));
    }

    #[test]
    fn hackernews_parses_story_item_extracts_title_score_url() {
        let json = item_fixture();
        let item = parse_hn_item(&json).unwrap();

        assert_eq!(item.id, 8863);
        assert_eq!(item.item_type, "story");
        assert_eq!(item.score, Some(111));
        assert_eq!(
            item.title.as_deref(),
            Some("My YC app: Dropbox - Throw away your USB drive")
        );
        assert_eq!(item.by.as_deref(), Some("dhouston"));
        assert_eq!(item.descendants, Some(71));
    }
}
