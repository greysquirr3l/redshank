//! GDELT — Global Event/Media Intelligence database.
//!
//! API: `https://api.gdeltproject.org/api/v2/doc/doc`
//! Params: `query={entity}&mode=artlist&format=json`
//! No auth required.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const API_URL: &str = "https://api.gdeltproject.org/api/v2/doc/doc";

/// Fetch GDELT media articles mentioning the given entity.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_gdelt_articles(
    entity: &str,
    output_dir: &Path,
    max_records: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let max = if max_records == 0 { 250 } else { max_records };

    let resp = client
        .get(API_URL)
        .query(&[
            ("query", entity),
            ("mode", "artlist"),
            ("format", "json"),
            ("maxrecords", &max.to_string()),
        ])
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let articles = json
        .get("articles")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let output_path = output_dir.join("gdelt_articles.ndjson");
    let count = write_ndjson(&output_path, &articles)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "gdelt".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    #[test]
    fn gdelt_parses_artlist_json_response() {
        let mock = serde_json::json!({
            "articles": [
                {
                    "url": "https://example.com/article1",
                    "title": "ACME Corp under investigation for fraud",
                    "seendate": "20240615T120000Z",
                    "socialimage": "https://example.com/img.jpg",
                    "domain": "example.com",
                    "language": "English",
                    "sourcecountry": "United States",
                    "tone": -3.5
                },
                {
                    "url": "https://news.example.org/article2",
                    "title": "ACME Corp settles lawsuit for $50M",
                    "seendate": "20240620T080000Z",
                    "socialimage": "",
                    "domain": "news.example.org",
                    "language": "English",
                    "sourcecountry": "United States",
                    "tone": -1.2
                }
            ]
        });
        let articles = mock.get("articles").and_then(|v| v.as_array()).unwrap();
        assert_eq!(articles.len(), 2);
        assert!(
            articles[0]["title"]
                .as_str()
                .unwrap()
                .contains("investigation")
        );
        assert_eq!(articles[0]["tone"], -3.5);
        assert_eq!(articles[1]["domain"], "news.example.org");
    }
}
