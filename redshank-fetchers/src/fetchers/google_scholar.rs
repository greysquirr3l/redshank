//! Google Scholar citation profile parser.

use crate::domain::{FetchError, FetchOutput};
use crate::write_ndjson;
use std::path::Path;

/// A top-cited paper from a Google Scholar profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ScholarPaper {
    /// Paper title.
    pub title: String,
    /// Journal or venue.
    pub venue: Option<String>,
    /// Publication year.
    pub year: Option<u16>,
    /// Citation count.
    pub citation_count: Option<u32>,
}

/// A co-author entry from a Google Scholar profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ScholarCoAuthor {
    /// Co-author display name.
    pub name: String,
    /// Linked profile id.
    pub profile_id: Option<String>,
}

/// A normalized Google Scholar profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GoogleScholarProfile {
    /// Scholar user id.
    pub user_id: String,
    /// Profile name.
    pub name: String,
    /// Affiliation text.
    pub affiliation: Option<String>,
    /// Verified email domain.
    pub verified_email_domain: Option<String>,
    /// All-time citations.
    pub total_citations: Option<u32>,
    /// All-time h-index.
    pub h_index: Option<u32>,
    /// All-time i10-index.
    pub i10_index: Option<u32>,
    /// Recent citations.
    pub recent_citations: Option<u32>,
    /// Recent h-index.
    pub recent_h_index: Option<u32>,
    /// Recent i10-index.
    pub recent_i10_index: Option<u32>,
    /// Top cited papers.
    pub top_papers: Vec<ScholarPaper>,
    /// Co-author links.
    pub coauthors: Vec<ScholarCoAuthor>,
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

/// Parse a Google Scholar profile fixture or cached HTML.
#[must_use]
pub fn parse_scholar_profile(user_id: &str, html: &str) -> Option<GoogleScholarProfile> {
    let name = extract_between(html, "data-scholar-name=\"", "\"")?;
    let affiliation = extract_between(html, "data-scholar-affiliation=\"", "\"");
    let verified_email_domain = extract_between(html, "data-scholar-email-domain=\"", "\"");

    let total_citations = extract_between(html, "data-metric-citations=\"", "\"")
        .and_then(|value| value.parse::<u32>().ok());
    let h_index = extract_between(html, "data-metric-hindex=\"", "\"")
        .and_then(|value| value.parse::<u32>().ok());
    let i10_index = extract_between(html, "data-metric-i10index=\"", "\"")
        .and_then(|value| value.parse::<u32>().ok());
    let recent_citations = extract_between(html, "data-metric-citations-5y=\"", "\"")
        .and_then(|value| value.parse::<u32>().ok());
    let recent_h_index = extract_between(html, "data-metric-hindex-5y=\"", "\"")
        .and_then(|value| value.parse::<u32>().ok());
    let recent_i10_index = extract_between(html, "data-metric-i10index-5y=\"", "\"")
        .and_then(|value| value.parse::<u32>().ok());

    let paper_titles = collect_attr_values(html, "data-paper-title");
    let paper_venues = collect_attr_values(html, "data-paper-venue");
    let paper_years = collect_attr_values(html, "data-paper-year");
    let paper_citations = collect_attr_values(html, "data-paper-citations");

    let top_papers = paper_titles
        .iter()
        .enumerate()
        .map(|(index, title)| ScholarPaper {
            title: title.clone(),
            venue: paper_venues.get(index).cloned(),
            year: paper_years
                .get(index)
                .and_then(|value| value.parse::<u16>().ok()),
            citation_count: paper_citations
                .get(index)
                .and_then(|value| value.parse::<u32>().ok()),
        })
        .collect();

    let coauthor_names = collect_attr_values(html, "data-coauthor-name");
    let coauthor_ids = collect_attr_values(html, "data-coauthor-id");
    let coauthors = coauthor_names
        .iter()
        .enumerate()
        .map(|(index, name)| ScholarCoAuthor {
            name: name.clone(),
            profile_id: coauthor_ids.get(index).cloned(),
        })
        .collect();

    Some(GoogleScholarProfile {
        user_id: user_id.to_string(),
        name,
        affiliation,
        verified_email_domain,
        total_citations,
        h_index,
        i10_index,
        recent_citations,
        recent_h_index,
        recent_i10_index,
        top_papers,
        coauthors,
    })
}

/// Persist a parsed Google Scholar profile.
///
/// # Errors
///
/// Returns `Err` if the profile cannot be parsed, serialized, or written.
pub fn save_scholar_profile(
    user_id: &str,
    html: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let profile = parse_scholar_profile(user_id, html)
        .ok_or_else(|| FetchError::Parse("could not parse Google Scholar profile".to_string()))?;
    let records =
        vec![serde_json::to_value(profile).map_err(|err| FetchError::Parse(err.to_string()))?];
    let output_path = output_dir.join("google_scholar.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "google_scholar".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn scholar_fixture() -> &'static str {
        r#"
        <html>
          <body>
            <main
              data-scholar-name="Dr. Elena Portman"
              data-scholar-affiliation="Institute for Network Analysis"
              data-scholar-email-domain="ina.edu"
              data-metric-citations="4321"
              data-metric-hindex="28"
              data-metric-i10index="41"
              data-metric-citations-5y="1875"
              data-metric-hindex-5y="19"
              data-metric-i10index-5y="24">
            </main>
            <div data-paper-title="Mapping Ownership Networks" data-paper-venue="Journal of Complex Systems" data-paper-year="2022" data-paper-citations="320"></div>
            <div data-paper-title="Sanctions Evasion Graphs" data-paper-venue="Risk & Compliance Review" data-paper-year="2024" data-paper-citations="118"></div>
            <div data-coauthor-name="Leah Kim" data-coauthor-id="coauthor_1"></div>
            <div data-coauthor-name="Noah Patel" data-coauthor-id="coauthor_2"></div>
          </body>
        </html>
        "#
    }

    #[test]
    fn google_scholar_parses_scholar_profile_fixture() {
        let profile = parse_scholar_profile("abc123", scholar_fixture()).unwrap();

        assert_eq!(profile.name, "Dr. Elena Portman");
        assert_eq!(
            profile.affiliation.as_deref(),
            Some("Institute for Network Analysis")
        );
        assert_eq!(profile.verified_email_domain.as_deref(), Some("ina.edu"));
        assert_eq!(profile.top_papers.len(), 2);
    }

    #[test]
    fn google_scholar_extracts_citation_metrics() {
        let profile = parse_scholar_profile("abc123", scholar_fixture()).unwrap();

        assert_eq!(profile.total_citations, Some(4321));
        assert_eq!(profile.h_index, Some(28));
        assert_eq!(profile.i10_index, Some(41));
        assert_eq!(profile.recent_h_index, Some(19));
        assert_eq!(profile.coauthors.len(), 2);
    }
}
