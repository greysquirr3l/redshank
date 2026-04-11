//! Amazon Author page parser.

use crate::domain::{FetchError, FetchOutput};
use crate::write_ndjson;
use std::path::Path;

/// A book listed on an Amazon author page.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AmazonBook {
    /// Book title.
    pub title: String,
    /// ASIN, if available.
    pub asin: Option<String>,
    /// Publication date.
    pub publication_date: Option<String>,
    /// Format label.
    pub format: Option<String>,
}

/// A normalized Amazon author page.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AmazonAuthorProfile {
    /// Author identifier or slug.
    pub author_id: String,
    /// Display name.
    pub name: String,
    /// Biography text.
    pub biography: Option<String>,
    /// Photo URL.
    pub photo_url: Option<String>,
    /// Followers count.
    pub followers_count: Option<u64>,
    /// Published books.
    pub books: Vec<AmazonBook>,
}

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let from = haystack.find(start)? + start.len();
    let remainder = haystack.get(from..)?;
    let to = remainder.find(end)?;
    Some(remainder[..to].trim().to_string())
}

fn collect_attr_values(html: &str, attr: &str) -> Vec<String> {
    let marker = format!("{attr}=\"");
    let mut values = Vec::new();
    let mut remainder = html;

    while let Some(idx) = remainder.find(&marker) {
        let after = &remainder[idx + marker.len()..];
        let Some(end_idx) = after.find('"') else {
            break;
        };
        let value = after[..end_idx].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        remainder = &after[end_idx + 1..];
    }

    values
}

/// Parse an Amazon author page fixture or cached HTML.
#[must_use]
pub fn parse_author_page(author_id: &str, html: &str) -> Option<AmazonAuthorProfile> {
    let name = extract_between(html, "data-author-name=\"", "\"")?;
    let biography = extract_between(html, "<section data-author-bio><p>", "</p>");
    let photo_url = extract_between(html, "data-author-photo=\"", "\"");
    let followers_count = extract_between(html, "data-followers=\"", "\"")
        .and_then(|value| value.parse::<u64>().ok());

    let titles = collect_attr_values(html, "data-book-title");
    let asins = collect_attr_values(html, "data-book-asin");
    let dates = collect_attr_values(html, "data-book-date");
    let formats = collect_attr_values(html, "data-book-format");

    let books = titles
        .iter()
        .enumerate()
        .map(|(index, title)| AmazonBook {
            title: title.clone(),
            asin: asins.get(index).cloned(),
            publication_date: dates.get(index).cloned(),
            format: formats.get(index).cloned(),
        })
        .collect();

    Some(AmazonAuthorProfile {
        author_id: author_id.to_string(),
        name,
        biography,
        photo_url,
        followers_count,
        books,
    })
}

/// Persist a parsed Amazon author profile.
///
/// # Errors
///
/// Returns `Err` if the profile cannot be parsed, serialized, or written.
pub async fn save_author_page(
    author_id: &str,
    html: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let profile = parse_author_page(author_id, html)
        .ok_or_else(|| FetchError::Parse("could not parse Amazon author page".to_string()))?;
    let records =
        vec![serde_json::to_value(profile).map_err(|err| FetchError::Parse(err.to_string()))?];
    let output_path = output_dir.join("amazon_authors.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "amazon_authors".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn author_fixture() -> &'static str {
        r#"
        <html>
          <body>
            <main data-author-name="Maya Ledger" data-author-photo="https://images.example.com/maya.jpg" data-followers="1284"></main>
            <section data-author-bio><p>Investigative journalist and author focused on illicit finance.</p></section>
            <div data-book-title="Shell Games" data-book-asin="B0TEST1234" data-book-date="2021-05-01" data-book-format="Kindle"></div>
            <div data-book-title="Following the Money" data-book-asin="B0TEST5678" data-book-date="2023-09-12" data-book-format="Hardcover"></div>
          </body>
        </html>
        "#
    }

    #[test]
    fn amazon_author_parses_author_page_fixture() {
        let profile = parse_author_page("maya-ledger", author_fixture()).unwrap();

        assert_eq!(profile.name, "Maya Ledger");
        assert_eq!(profile.followers_count, Some(1284));
        assert_eq!(
            profile.photo_url.as_deref(),
            Some("https://images.example.com/maya.jpg")
        );
    }

    #[test]
    fn amazon_author_extracts_biography_and_book_list() {
        let profile = parse_author_page("maya-ledger", author_fixture()).unwrap();

        assert!(
            profile
                .biography
                .as_deref()
                .unwrap()
                .contains("illicit finance")
        );
        assert_eq!(profile.books.len(), 2);
        assert_eq!(profile.books[0].title, "Shell Games");
        assert_eq!(profile.books[1].format.as_deref(), Some("Hardcover"));
    }
}
