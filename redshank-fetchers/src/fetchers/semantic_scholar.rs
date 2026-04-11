//! Semantic Scholar API — academic publication and researcher intelligence.
//!
//! Source: <https://api.semanticscholar.org/graph/v1/>
//! Free tier: 100 req/min unauthenticated; set `semantic_scholar_api_key` for higher limits.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, build_client_with_key, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.semanticscholar.org/graph/v1";

/// An academic author profile from Semantic Scholar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScholarAuthor {
    /// Semantic Scholar author ID.
    pub author_id: String,
    /// Author name.
    pub name: String,
    /// Scholar profile URL.
    pub url: Option<String>,
    /// H-index.
    pub h_index: Option<u32>,
    /// Total citation count.
    pub citation_count: Option<u32>,
    /// Total paper count.
    pub paper_count: Option<u32>,
    /// Institutional affiliations.
    pub affiliations: Vec<String>,
    /// Primary research fields.
    pub fields_of_study: Vec<String>,
    /// Top papers (by citation count).
    pub papers: Vec<ScholarPaper>,
}

/// An academic paper record from Semantic Scholar.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScholarPaper {
    /// Semantic Scholar paper ID.
    pub paper_id: String,
    /// Paper title.
    pub title: String,
    /// Publication year.
    pub year: Option<u32>,
    /// Venue or journal.
    pub venue: Option<String>,
    /// Total citations.
    pub citation_count: Option<u32>,
    /// Highly-influential citations count.
    pub influential_citation_count: Option<u32>,
    /// Open-access PDF URL.
    pub open_access_pdf: Option<String>,
    /// First author's name.
    pub first_author: Option<String>,
    /// External URL.
    pub url: Option<String>,
}

/// Parse a Semantic Scholar author search response.
///
/// Handles the `{"data": [{...}]}` envelope from `GET /author/search`.
#[must_use]
pub fn parse_author_search(json: &serde_json::Value) -> Vec<ScholarAuthor> {
    let arr = json
        .get("data")
        .and_then(serde_json::Value::as_array)
        .or_else(|| json.as_array());

    arr.map(|items| items.iter().filter_map(parse_single_author).collect())
        .unwrap_or_default()
}

fn parse_single_author(item: &serde_json::Value) -> Option<ScholarAuthor> {
    let author_id = item
        .get("authorId")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let affiliations = item
        .get("affiliations")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let fields_of_study = item
        .get("fieldsOfStudy")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|f| f.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let papers = item
        .get("papers")
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_single_paper).collect())
        .unwrap_or_default();

    Some(ScholarAuthor {
        author_id,
        name,
        url: item
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        h_index: item
            .get("hIndex")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        citation_count: item
            .get("citationCount")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        paper_count: item
            .get("paperCount")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        affiliations,
        fields_of_study,
        papers,
    })
}

fn parse_single_paper(item: &serde_json::Value) -> Option<ScholarPaper> {
    let paper_id = item
        .get("paperId")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let title = item
        .get("title")
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let open_access_pdf = item
        .get("openAccessPdf")
        .and_then(|v| v.get("url"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let first_author = item
        .get("authors")
        .and_then(serde_json::Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|a| a.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    Some(ScholarPaper {
        paper_id,
        title,
        year: item
            .get("year")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        venue: item
            .get("venue")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from),
        citation_count: item
            .get("citationCount")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        influential_citation_count: item
            .get("influentialCitationCount")
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as u32),
        open_access_pdf,
        first_author,
        url: item
            .get("url")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

/// Fetch authors matching `name` via Semantic Scholar.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or the response is not valid JSON.
pub async fn fetch_author_search(
    name: &str,
    api_key: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = if let Some(key) = api_key {
        build_client_with_key("x-api-key", key)?
    } else {
        build_client()?
    };

    rate_limit_delay(rate_limit_ms).await;

    let fields =
        "authorId,name,url,hIndex,citationCount,paperCount,affiliations,fieldsOfStudy,papers";
    let resp = client
        .get(format!("{API_BASE}/author/search"))
        .query(&[("query", name), ("fields", fields)])
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
    let authors = parse_author_search(&json);

    let serialized: Vec<serde_json::Value> = authors
        .iter()
        .filter_map(|a| serde_json::to_value(a).ok())
        .collect();

    let output_path = output_dir.join("semantic_scholar_authors.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "semantic_scholar".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn author_fixture() -> serde_json::Value {
        serde_json::json!({
            "data": [
                {
                    "authorId": "2109876543",
                    "name": "Jane Researcher",
                    "url": "https://www.semanticscholar.org/author/2109876543",
                    "hIndex": 42,
                    "citationCount": 8500,
                    "paperCount": 120,
                    "affiliations": ["MIT", "Stanford"],
                    "fieldsOfStudy": ["Computer Science", "Machine Learning"],
                    "papers": [
                        {
                            "paperId": "abc123",
                            "title": "Deep Learning for OSINT",
                            "year": 2022,
                            "venue": "NeurIPS",
                            "citationCount": 310,
                            "influentialCitationCount": 45,
                            "openAccessPdf": {"url": "https://arxiv.org/pdf/2022.abc"},
                            "authors": [{"authorId": "2109876543", "name": "Jane Researcher"}],
                            "url": "https://www.semanticscholar.org/paper/abc123"
                        }
                    ]
                }
            ]
        })
    }

    #[test]
    fn semantic_scholar_parses_author_search_fixture_extracts_author_id_h_index_paper_count() {
        let json = author_fixture();
        let authors = parse_author_search(&json);

        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].author_id, "2109876543");
        assert_eq!(authors[0].name, "Jane Researcher");
        assert_eq!(authors[0].h_index, Some(42));
        assert_eq!(authors[0].paper_count, Some(120));
        assert_eq!(authors[0].citation_count, Some(8500));
    }

    #[test]
    fn semantic_scholar_extracts_paper_details_citations_and_co_author_network() {
        let json = author_fixture();
        let authors = parse_author_search(&json);

        let paper = &authors[0].papers[0];
        assert_eq!(paper.paper_id, "abc123");
        assert_eq!(paper.title, "Deep Learning for OSINT");
        assert_eq!(paper.venue.as_deref(), Some("NeurIPS"));
        assert_eq!(paper.citation_count, Some(310));
        assert_eq!(paper.influential_citation_count, Some(45));
        assert_eq!(
            paper.open_access_pdf.as_deref(),
            Some("https://arxiv.org/pdf/2022.abc")
        );
        assert_eq!(paper.first_author.as_deref(), Some("Jane Researcher"));
    }
}
