//! Stack Exchange profile lookup via the public Stack Exchange API.
//!
//! This fetcher searches public Stack Overflow users by display-name fragment.
//! Optional API key support can be added later for higher quota.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const STACKEXCHANGE_API: &str = "https://api.stackexchange.com/2.3/users";

fn map_stackexchange_item(item: &serde_json::Value, normalized: &str) -> serde_json::Value {
    serde_json::json!({
        "query": normalized,
        "source": "stackexchange",
        "site": "stackoverflow",
        "user_id": item.get("user_id").and_then(serde_json::Value::as_i64),
        "display_name": item.get("display_name").and_then(serde_json::Value::as_str),
        "profile_image": item.get("profile_image").and_then(serde_json::Value::as_str),
        "profile_url": item.get("link").and_then(serde_json::Value::as_str),
        "reputation": item.get("reputation").and_then(serde_json::Value::as_i64),
        "badge_counts": item.get("badge_counts"),
        "creation_date": item.get("creation_date").and_then(serde_json::Value::as_i64),
        "last_access_date": item.get("last_access_date").and_then(serde_json::Value::as_i64),
        "location": item.get("location").and_then(serde_json::Value::as_str),
    })
}

/// Fetch Stack Exchange public profiles that match a display-name query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the API returns a non-success status,
/// or the response cannot be parsed/written.
pub async fn fetch_stackexchange_profile(
    query: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let normalized = query.trim();
    if normalized.is_empty() {
        return Err(FetchError::Parse(
            "query must not be empty for stackexchange_profile".to_string(),
        ));
    }

    let client = build_client()?;
    let response = client
        .get(STACKEXCHANGE_API)
        .query(&[
            ("site", "stackoverflow"),
            ("inname", normalized),
            ("pagesize", "20"),
            ("order", "desc"),
            ("sort", "reputation"),
        ])
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let payload: serde_json::Value = response.json().await?;
    let records = payload
        .get("items")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| map_stackexchange_item(item, normalized))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let output_path = output_dir.join("stackexchange_profile.ndjson");
    let records_written = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written,
        output_path,
        source_name: "stackexchange_profile".to_string(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_query_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = fetch_stackexchange_profile("   ", dir.path())
            .await
            .unwrap_err();
        assert!(matches!(err, FetchError::Parse(_)));
    }

    #[test]
    fn map_stackexchange_item_shapes_expected_fields() {
        let item = serde_json::json!({
            "user_id": 123,
            "display_name": "Jane Dev",
            "link": "https://stackoverflow.com/users/123/jane-dev",
            "reputation": 9999,
            "badge_counts": {"gold": 1, "silver": 2, "bronze": 3}
        });

        let mapped = map_stackexchange_item(&item, "jane");
        assert_eq!(mapped.get("query").and_then(serde_json::Value::as_str), Some("jane"));
        assert_eq!(
            mapped.get("source").and_then(serde_json::Value::as_str),
            Some("stackexchange")
        );
        assert_eq!(
            mapped.get("site").and_then(serde_json::Value::as_str),
            Some("stackoverflow")
        );
        assert_eq!(mapped.get("user_id").and_then(serde_json::Value::as_i64), Some(123));
        assert_eq!(
            mapped
                .get("display_name")
                .and_then(serde_json::Value::as_str),
            Some("Jane Dev")
        );
        assert_eq!(
            mapped
                .get("profile_url")
                .and_then(serde_json::Value::as_str),
            Some("https://stackoverflow.com/users/123/jane-dev")
        );
    }
}
