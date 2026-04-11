//! YouTube Data API v3 — channel and video metadata intelligence.
//!
//! Source: <https://www.googleapis.com/youtube/v3/>
//! Requires `youtube_api_key` in the credential store. Free tier: 10,000 units/day.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://www.googleapis.com/youtube/v3";

/// A YouTube channel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct YoutubeChannel {
    /// YouTube channel ID.
    pub channel_id: String,
    /// Channel title.
    pub title: String,
    /// Channel description.
    pub description: Option<String>,
    /// Custom URL slug (e.g. "@channelname").
    pub custom_url: Option<String>,
    /// Subscriber count.
    pub subscriber_count: Option<u64>,
    /// Total video count.
    pub video_count: Option<u64>,
    /// Total view count.
    pub view_count: Option<u64>,
    /// Publication date (ISO 8601).
    pub published_at: Option<String>,
    /// Country code.
    pub country: Option<String>,
}

/// A YouTube video.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct YoutubeVideo {
    /// Video ID.
    pub video_id: String,
    /// Channel ID.
    pub channel_id: Option<String>,
    /// Video title.
    pub title: String,
    /// Video description.
    pub description: Option<String>,
    /// Publish timestamp (ISO 8601).
    pub published_at: Option<String>,
    /// View count.
    pub view_count: Option<u64>,
    /// Like count.
    pub like_count: Option<u64>,
    /// Comment count.
    pub comment_count: Option<u64>,
    /// Duration (ISO 8601, e.g. "PT5M30S").
    pub duration: Option<String>,
    /// Category ID.
    pub category_id: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
}

/// Parse a YouTube channel list response (`.items[].snippet` + `.statistics`).
#[must_use]
pub fn parse_channel_list(json: &serde_json::Value) -> Vec<YoutubeChannel> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_single_channel).collect())
        .unwrap_or_default()
}

fn parse_single_channel(item: &serde_json::Value) -> Option<YoutubeChannel> {
    let channel_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let snippet = item.get("snippet")?;
    let title = snippet
        .get("title")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let stats = item.get("statistics");
    let stat_u64 = |key: &str| -> Option<u64> {
        stats.and_then(|s| s.get(key)).and_then(|v| {
            // YouTube returns counts as strings
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| v.as_u64())
        })
    };

    Some(YoutubeChannel {
        channel_id,
        title,
        description: snippet
            .get("description")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        custom_url: snippet
            .get("customUrl")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        subscriber_count: stat_u64("subscriberCount"),
        video_count: stat_u64("videoCount"),
        view_count: stat_u64("viewCount"),
        published_at: snippet
            .get("publishedAt")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        country: snippet
            .get("country")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

/// Parse a YouTube video list response.
#[must_use]
pub fn parse_video_list(json: &serde_json::Value) -> Vec<YoutubeVideo> {
    json.get("items")
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_single_video).collect())
        .unwrap_or_default()
}

fn parse_single_video(item: &serde_json::Value) -> Option<YoutubeVideo> {
    let video_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let snippet = item.get("snippet")?;
    let title = snippet
        .get("title")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let stats = item.get("statistics");
    let stat_u64 = |key: &str| -> Option<u64> {
        stats.and_then(|s| s.get(key)).and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or_else(|| v.as_u64())
        })
    };

    let tags = snippet
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Some(YoutubeVideo {
        video_id,
        channel_id: snippet
            .get("channelId")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        title,
        description: snippet
            .get("description")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        published_at: snippet
            .get("publishedAt")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        view_count: stat_u64("viewCount"),
        like_count: stat_u64("likeCount"),
        comment_count: stat_u64("commentCount"),
        duration: item
            .get("contentDetails")
            .and_then(|cd| cd.get("duration"))
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        category_id: snippet
            .get("categoryId")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        tags,
    })
}

/// Fetch YouTube channel and video data by search query.
///
/// # Errors
///
/// Returns `Err` if the API key is invalid or the request fails.
pub async fn fetch_youtube_channel(
    query: &str,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    // Search for channels matching the query
    let search_resp = client
        .get(format!("{API_BASE}/search"))
        .query(&[
            ("q", query),
            ("type", "channel"),
            ("part", "snippet"),
            ("maxResults", "10"),
            ("key", api_key),
        ])
        .send()
        .await?;

    let status = search_resp.status();
    if !status.is_success() {
        let body = search_resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let search_json: serde_json::Value = search_resp.json().await?;

    // Extract channel IDs from search results
    let channel_ids: Vec<String> = search_json
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    item.get("id")
                        .and_then(|id| id.get("channelId"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    if channel_ids.is_empty() {
        let output_path = output_dir.join("youtube_channels.ndjson");
        write_ndjson(&output_path, &[])?;
        return Ok(FetchOutput {
            records_written: 0,
            output_path,
            source_name: "youtube".into(),
            attribution: None,
        });
    }

    rate_limit_delay(rate_limit_ms).await;

    // Fetch full channel details
    let ids_str = channel_ids.join(",");
    let channels_resp = client
        .get(format!("{API_BASE}/channels"))
        .query(&[
            ("id", ids_str.as_str()),
            ("part", "snippet,statistics"),
            ("key", api_key),
        ])
        .send()
        .await?;

    let channels_json: serde_json::Value = channels_resp.json().await?;
    let channels = parse_channel_list(&channels_json);

    let serialized: Vec<serde_json::Value> = channels
        .iter()
        .filter_map(|c| serde_json::to_value(c).ok())
        .collect();

    let output_path = output_dir.join("youtube_channels.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "youtube".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn channel_fixture() -> serde_json::Value {
        serde_json::json!({
            "items": [
                {
                    "id": "UCxxxxYYYYZZZZ",
                    "snippet": {
                        "title": "Investigative Tech",
                        "description": "Channel about data journalism and OSINT",
                        "customUrl": "@investigativetech",
                        "publishedAt": "2018-03-15T00:00:00Z",
                        "country": "US"
                    },
                    "statistics": {
                        "subscriberCount": "45000",
                        "videoCount": "312",
                        "viewCount": "8500000"
                    }
                }
            ]
        })
    }

    fn video_fixture() -> serde_json::Value {
        serde_json::json!({
            "items": [
                {
                    "id": "dQw4w9WgXcQ",
                    "snippet": {
                        "channelId": "UCxxxxYYYYZZZZ",
                        "title": "How to Use OSINT for Investigations",
                        "description": "A deep dive into open-source intelligence techniques",
                        "publishedAt": "2023-06-01T12:00:00Z",
                        "categoryId": "28",
                        "tags": ["OSINT", "investigations", "data journalism"]
                    },
                    "statistics": {
                        "viewCount": "125000",
                        "likeCount": "4200",
                        "commentCount": "350"
                    },
                    "contentDetails": {
                        "duration": "PT22M15S"
                    }
                }
            ]
        })
    }

    #[test]
    fn youtube_parses_channel_fixture_extracts_metadata_subscriber_video_counts() {
        let json = channel_fixture();
        let channels = parse_channel_list(&json);

        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].channel_id, "UCxxxxYYYYZZZZ");
        assert_eq!(channels[0].title, "Investigative Tech");
        assert_eq!(channels[0].subscriber_count, Some(45_000));
        assert_eq!(channels[0].video_count, Some(312));
        assert_eq!(
            channels[0].custom_url.as_deref(),
            Some("@investigativetech")
        );
    }

    #[test]
    fn youtube_parses_video_fixture_extracts_title_view_count_duration_and_tags() {
        let json = video_fixture();
        let videos = parse_video_list(&json);

        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].video_id, "dQw4w9WgXcQ");
        assert_eq!(videos[0].title, "How to Use OSINT for Investigations");
        assert_eq!(videos[0].view_count, Some(125_000));
        assert_eq!(videos[0].duration.as_deref(), Some("PT22M15S"));
        assert!(videos[0].tags.contains(&"OSINT".to_string()));
    }
}
