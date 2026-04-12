//! GuideStar / Candid nonprofit profile parser and fetch helper.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client_with_key, write_ndjson};
use std::path::Path;

/// A normalized nonprofit leadership record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct NonprofitLeader {
    /// Leader name.
    pub name: String,
    /// Role or title.
    pub title: Option<String>,
    /// Compensation amount if reported.
    pub compensation: Option<f64>,
}

/// A normalized GuideStar / Candid profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GuidestarCandidProfile {
    /// Organization EIN.
    pub ein: String,
    /// Organization name.
    pub name: String,
    /// Mission text.
    pub mission: Option<String>,
    /// IRS subsection classification.
    pub irs_subsection: Option<String>,
    /// Seal of Transparency level.
    pub transparency_seal: Option<String>,
    /// DEI statement summary.
    pub dei_statement: Option<String>,
    /// Leaders and compensation.
    pub leadership: Vec<NonprofitLeader>,
    /// Charity status or due-diligence status.
    pub status: Option<String>,
    /// OFAC or terrorism screening status.
    pub sanctions_check: Option<String>,
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

/// Parse a GuideStar / Candid profile fixture or cached HTML.
#[must_use]
pub fn parse_guidestar_profile(ein: &str, html: &str) -> Option<GuidestarCandidProfile> {
    let name = extract_between(html, "data-org-name=\"", "\"")?;
    let leader_names = collect_attr_values(html, "data-leader-name");
    let leader_titles = collect_attr_values(html, "data-leader-title");
    let leader_comp = collect_attr_values(html, "data-leader-compensation");
    let leadership = leader_names
        .iter()
        .enumerate()
        .map(|(index, name)| NonprofitLeader {
            name: name.clone(),
            title: leader_titles.get(index).cloned(),
            compensation: leader_comp
                .get(index)
                .and_then(|value| value.replace(',', "").parse::<f64>().ok()),
        })
        .collect();

    Some(GuidestarCandidProfile {
        ein: ein.to_string(),
        name,
        mission: extract_between(html, "data-mission=\"", "\""),
        irs_subsection: extract_between(html, "data-subsection=\"", "\""),
        transparency_seal: extract_between(html, "data-transparency-seal=\"", "\""),
        dei_statement: extract_between(html, "data-dei-statement=\"", "\""),
        leadership,
        status: extract_between(html, "data-charity-status=\"", "\""),
        sanctions_check: extract_between(html, "data-sanctions-check=\"", "\""),
    })
}

/// Fetch and persist a GuideStar/Candid profile using an API key for authorization.
///
/// # Errors
///
/// Returns `Err` if the request fails, the server rejects the key, or parsing fails.
pub async fn fetch_guidestar_profile(
    ein: &str,
    api_key: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client_with_key("x-api-key", api_key)?;
    let url = format!("https://www.guidestar.org/profile/{ein}");
    let resp = client.get(&url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let body = resp.text().await?;
    let profile = parse_guidestar_profile(ein, &body)
        .ok_or_else(|| FetchError::Parse("could not parse GuideStar/Candid profile".to_string()))?;
    let records = vec![serde_json::to_value(profile)
        .map_err(|err| FetchError::Parse(err.to_string()))?];
    let output_path = output_dir.join("guidestar_candid.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "guidestar-candid".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn profile_fixture() -> &'static str {
        r#"
        <html>
          <body>
            <main data-org-name="Open Civic Data Trust" data-mission="Strengthen nonprofit accountability through open data." data-subsection="501(c)(3)" data-transparency-seal="Platinum" data-dei-statement="Board-approved DEI commitment published in 2024." data-charity-status="Active" data-sanctions-check="No matches found"></main>
            <div data-leader-name="Dana Mercer" data-leader-title="CEO" data-leader-compensation="245000"></div>
            <div data-leader-name="Liam Okafor" data-leader-title="CFO" data-leader-compensation="198500"></div>
          </body>
        </html>
        "#
    }

    #[test]
    fn guidestar_parses_profile_fixture() {
        let profile = parse_guidestar_profile("112233445", profile_fixture()).unwrap();

        assert_eq!(profile.name, "Open Civic Data Trust");
        assert_eq!(profile.irs_subsection.as_deref(), Some("501(c)(3)"));
        assert_eq!(profile.leadership.len(), 2);
        assert_eq!(profile.status.as_deref(), Some("Active"));
    }

    #[test]
    fn guidestar_extracts_transparency_and_ceo_compensation() {
        let profile = parse_guidestar_profile("112233445", profile_fixture()).unwrap();

        assert_eq!(profile.transparency_seal.as_deref(), Some("Platinum"));
        assert_eq!(profile.leadership[0].title.as_deref(), Some("CEO"));
        assert_eq!(profile.leadership[0].compensation, Some(245_000.0));
        assert!(profile
            .dei_statement
            .as_deref()
            .unwrap()
            .contains("DEI commitment"));
    }
}