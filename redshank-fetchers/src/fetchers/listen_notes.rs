//! Listen Notes API — podcast and episode discovery.
//!
//! Source: <https://listen-api.listennotes.com/api/v2/>
//! Requires `listennotes_api_key` in the credential store.
//! Free tier: 500 req/month — sufficient for targeted searches.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client_with_key, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://listen-api.listennotes.com/api/v2";

/// A podcast series.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Podcast {
    /// Listen Notes podcast ID.
    pub id: String,
    /// Podcast title.
    pub title: String,
    /// Publisher / producer name.
    pub publisher: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// RSS feed URL.
    pub rss_feed: Option<String>,
    /// iTunes / Apple Podcasts ID.
    pub itunes_id: Option<i64>,
    /// Total episode count.
    pub total_episodes: Option<u32>,
    /// Latest publish date (Unix timestamp).
    pub latest_pub_date_ms: Option<i64>,
    /// Listen Notes profile URL.
    pub listennotes_url: Option<String>,
}

/// A podcast episode.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PodcastEpisode {
    /// Listen Notes episode ID.
    pub id: String,
    /// Episode title.
    pub title: String,
    /// Episode description (may include guest names).
    pub description: Option<String>,
    /// Publish date (Unix timestamp in ms).
    pub pub_date_ms: Option<i64>,
    /// Audio URL.
    pub audio: Option<String>,
    /// Duration in seconds.
    pub audio_length_sec: Option<u32>,
    /// Podcast the episode belongs to.
    pub podcast_id: Option<String>,
    /// Podcast title.
    pub podcast_title: Option<String>,
    /// Listen Notes episode URL.
    pub listennotes_url: Option<String>,
}

/// Parse a Listen Notes search response.
///
/// Handles both `{"results": [{...}]}` (episode/podcast search) shapes.
#[must_use]
pub fn parse_search_results(json: &serde_json::Value) -> (Vec<Podcast>, Vec<PodcastEpisode>) {
    let results = json
        .get("results")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut podcasts = Vec::new();
    let mut episodes = Vec::new();

    for item in &results {
        let type_hint = item
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");

        if type_hint == "podcast" || item.get("total_episodes").is_some() {
            if let Some(p) = parse_podcast_item(item) {
                podcasts.push(p);
            }
        } else if let Some(e) = parse_episode_item(item) {
            episodes.push(e);
        }
    }

    (podcasts, episodes)
}

fn parse_podcast_item(item: &serde_json::Value) -> Option<Podcast> {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let title = item
        .get("title")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    Some(Podcast {
        id,
        title,
        publisher: item
            .get("publisher")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        description: item
            .get("description")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        rss_feed: item
            .get("rss")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        itunes_id: item.get("itunes_id").and_then(serde_json::Value::as_i64),
        total_episodes: item
            .get("total_episodes")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        latest_pub_date_ms: item
            .get("latest_pub_date_ms")
            .and_then(serde_json::Value::as_i64),
        listennotes_url: item
            .get("listennotes_url")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

fn parse_episode_item(item: &serde_json::Value) -> Option<PodcastEpisode> {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let title = item
        .get("title")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    // Podcast details may be nested under "podcast" sub-object
    let podcast_obj = item.get("podcast");
    let podcast_id = podcast_obj
        .and_then(|p| p.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);
    let podcast_title = podcast_obj
        .and_then(|p| p.get("title"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    Some(PodcastEpisode {
        id,
        title,
        description: item
            .get("description")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        pub_date_ms: item.get("pub_date_ms").and_then(serde_json::Value::as_i64),
        audio: item
            .get("audio")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        audio_length_sec: item
            .get("audio_length_sec")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        podcast_id,
        podcast_title,
        listennotes_url: item
            .get("listennotes_url")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

/// Fetch podcast/episode search results from Listen Notes.
///
/// # Errors
///
/// Returns `Err` if the API request fails or returns a non-2xx status.
pub async fn fetch_listen_notes(
    query: &str,
    episode_type: bool,
    api_key: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client_with_key("X-ListenAPI-Key", api_key)?;
    rate_limit_delay(rate_limit_ms).await;

    let search_type = if episode_type { "episode" } else { "podcast" };
    let resp = client
        .get(format!("{API_BASE}/search"))
        .query(&[("q", query), ("type", search_type), ("page_size", "10")])
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
    let (podcasts, episodes) = parse_search_results(&json);

    let mut records: Vec<serde_json::Value> = Vec::new();
    for p in &podcasts {
        if let Ok(v) = serde_json::to_value(p) {
            records.push(v);
        }
    }
    for e in &episodes {
        if let Ok(v) = serde_json::to_value(e) {
            records.push(v);
        }
    }

    let slug = query
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .take(40)
        .collect::<String>();
    let output_path = output_dir.join(format!("listennotes_{slug}.ndjson"));
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "listen_notes".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn podcast_search_fixture() -> serde_json::Value {
        serde_json::json!({
            "results": [
                {
                    "id": "abc123podcast",
                    "type": "podcast",
                    "title": "The OSINT Hour",
                    "publisher": "Investigative Media LLC",
                    "description": "Weekly deep dives into open-source intelligence techniques",
                    "rss": "https://feeds.buzzsprout.com/osinthour.rss",
                    "itunes_id": 1234567890,
                    "total_episodes": 87,
                    "latest_pub_date_ms": 1_700_000_000_000_i64,
                    "listennotes_url": "https://www.listennotes.com/podcasts/the-osint-hour-abc123"
                }
            ]
        })
    }

    fn episode_search_fixture() -> serde_json::Value {
        serde_json::json!({
            "results": [
                {
                    "id": "ep456",
                    "title": "Tracking Shell Companies with John Doe",
                    "description": "Guest John Doe, former Treasury analyst, discusses FATF recommendations and corporate registry gaps",
                    "pub_date_ms": 1_695_000_000_000_i64,
                    "audio": "https://media.buzzsprout.com/ep456.mp3",
                    "audio_length_sec": 3245,
                    "podcast": {
                        "id": "abc123podcast",
                        "title": "The OSINT Hour"
                    },
                    "listennotes_url": "https://www.listennotes.com/episodes/ep456"
                }
            ]
        })
    }

    #[test]
    fn listennotes_parses_podcast_search_results_extracts_title_publisher_rss() {
        let json = podcast_search_fixture();
        let (podcasts, episodes) = parse_search_results(&json);

        assert_eq!(podcasts.len(), 1);
        assert!(episodes.is_empty());
        assert_eq!(podcasts[0].id, "abc123podcast");
        assert_eq!(podcasts[0].title, "The OSINT Hour");
        assert_eq!(
            podcasts[0].publisher.as_deref(),
            Some("Investigative Media LLC")
        );
        assert_eq!(podcasts[0].total_episodes, Some(87));
        assert!(
            podcasts[0]
                .rss_feed
                .as_deref()
                .unwrap()
                .contains("buzzsprout")
        );
    }

    #[test]
    fn listennotes_parses_episode_results_extracts_guest_names_from_description() {
        let json = episode_search_fixture();
        let (podcasts, episodes) = parse_search_results(&json);

        assert_eq!(episodes.len(), 1);
        assert!(podcasts.is_empty());
        assert_eq!(episodes[0].title, "Tracking Shell Companies with John Doe");
        assert!(
            episodes[0]
                .description
                .as_deref()
                .unwrap()
                .contains("John Doe")
        );
        assert_eq!(episodes[0].audio_length_sec, Some(3245));
        assert_eq!(episodes[0].podcast_title.as_deref(), Some("The OSINT Hour"));
    }
}
