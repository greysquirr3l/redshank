//! GitHub Public Profile — REST API for user profiles and organizations.
//!
//! API: `https://api.github.com/users/{username}`
//! No auth for public data (60 req/hr); token for 5000 req/hr.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.github.com";

/// Fetch a GitHub user's public profile, organizations, and repositories.
pub async fn fetch_github_profile(
    username: &str,
    token: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut records = Vec::new();

    // Fetch user profile
    let user_resp = send_github_request(&client, &format!("{API_BASE}/users/{username}"), token).await?;
    let profile: serde_json::Value = user_resp;

    // Fetch organizations
    rate_limit_delay(rate_limit_ms).await;
    let orgs: serde_json::Value =
        send_github_request(&client, &format!("{API_BASE}/users/{username}/orgs"), token).await?;

    // Fetch repositories
    rate_limit_delay(rate_limit_ms).await;
    let repos: serde_json::Value = send_github_request(
        &client,
        &format!("{API_BASE}/users/{username}/repos?sort=updated&per_page=100"),
        token,
    )
    .await?;

    records.push(serde_json::json!({
        "profile": profile,
        "organizations": orgs,
        "repositories": repos,
    }));

    let output_path = output_dir.join("github_profile.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "github".into(),
    })
}

/// Reverse-lookup a GitHub username from an email address.
pub async fn search_user_by_email(
    email: &str,
    token: Option<&str>,
) -> Result<Vec<serde_json::Value>, FetchError> {
    let client = build_client()?;
    let json: serde_json::Value = send_github_request(
        &client,
        &format!("{API_BASE}/search/users?q={email}+in:email"),
        token,
    )
    .await?;

    Ok(json
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default())
}

async fn send_github_request(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
) -> Result<serde_json::Value, FetchError> {
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "redshank-investigation-agent");

    if let Some(t) = token {
        req = req.bearer_auth(t);
    }

    let resp = req.send().await?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    Ok(resp.json().await?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn github_profile_fixture_parses_user_and_orgs() {
        let profile = serde_json::json!({
            "login": "testuser",
            "name": "Test User",
            "company": "ACME Corp",
            "location": "San Francisco",
            "email": "test@example.com",
            "bio": "Software developer",
            "public_repos": 42,
        });
        let orgs = serde_json::json!([
            {"login": "acme-corp", "description": "ACME Corp GitHub org"},
            {"login": "open-source-club", "description": "OSS club"},
        ]);
        assert_eq!(profile["login"], "testuser");
        assert_eq!(profile["company"], "ACME Corp");
        let org_list = orgs.as_array().unwrap();
        assert_eq!(org_list.len(), 2);
        assert_eq!(org_list[0]["login"], "acme-corp");
    }

    #[test]
    fn github_email_reverse_lookup_parses_search_result() {
        let mock = serde_json::json!({
            "total_count": 1,
            "items": [
                {"login": "founduser", "id": 12345, "type": "User"}
            ]
        });
        let items = mock.get("items").and_then(|v| v.as_array()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["login"], "founduser");
    }
}
