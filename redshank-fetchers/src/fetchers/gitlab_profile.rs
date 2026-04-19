//! GitLab profile lookup via the public GitLab Users API.
//!
//! Uses unauthenticated public search by default.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const GITLAB_USERS_API: &str = "https://gitlab.com/api/v4/users";

/// Fetch GitLab public profiles that match a search query.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the API returns a non-success status,
/// or the response cannot be parsed/written.
pub async fn fetch_gitlab_profile(
    query: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let normalized = query.trim();
    if normalized.is_empty() {
        return Err(FetchError::Parse(
            "query must not be empty for gitlab_profile".to_string(),
        ));
    }

    let client = build_client()?;
    let response = client
        .get(GITLAB_USERS_API)
        .query(&[("search", normalized), ("per_page", "20")])
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
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "query": normalized,
                        "source": "gitlab",
                        "id": item.get("id").and_then(serde_json::Value::as_i64),
                        "username": item.get("username").and_then(serde_json::Value::as_str),
                        "name": item.get("name").and_then(serde_json::Value::as_str),
                        "state": item.get("state").and_then(serde_json::Value::as_str),
                        "web_url": item.get("web_url").and_then(serde_json::Value::as_str),
                        "avatar_url": item.get("avatar_url").and_then(serde_json::Value::as_str),
                        "bio": item.get("bio").and_then(serde_json::Value::as_str),
                        "location": item.get("location").and_then(serde_json::Value::as_str),
                        "public_email": item.get("public_email").and_then(serde_json::Value::as_str),
                        "created_at": item.get("created_at").and_then(serde_json::Value::as_str),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let output_path = output_dir.join("gitlab_profile.ndjson");
    let records_written = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written,
        output_path,
        source_name: "gitlab_profile".to_string(),
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
        let err = fetch_gitlab_profile("\n", dir.path()).await.unwrap_err();
        assert!(matches!(err, FetchError::Parse(_)));
    }
}
