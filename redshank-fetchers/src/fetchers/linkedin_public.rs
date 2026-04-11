//! LinkedIn public profile scraping helpers.
//!
//! Only publicly visible profiles should be accessed. This module parses
//! already-fetched HTML and exposes a lightweight pipeline config parser so the
//! browser-backed workflow can be validated without launching a browser.

use crate::domain::{FetchError, FetchOutput};
use crate::write_ndjson;
use std::path::Path;

/// Pipeline config loaded at compile time.
pub const PIPELINE_CONFIG: &str = include_str!("../../pipelines/linkedin_public/config.toml");

/// Minimum delay between LinkedIn public profile fetches.
pub const PROFILE_DELAY_MS: u64 = 30_000;

/// A parsed LinkedIn pipeline config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedInPipelineConfig {
    pub name: String,
    pub rate_limit_seconds: u64,
    pub requires_browser: bool,
    pub wait_for: String,
}

/// A single LinkedIn work history item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LinkedInPosition {
    pub title: String,
    pub company: String,
    pub dates: Option<String>,
    pub description: Option<String>,
}

/// A single LinkedIn education entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LinkedInEducation {
    pub school: String,
    pub degree: Option<String>,
    pub field: Option<String>,
    pub dates: Option<String>,
}

/// A normalized LinkedIn public profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LinkedInProfile {
    pub slug: String,
    pub name: String,
    pub headline: Option<String>,
    pub current_company: Option<String>,
    pub location: Option<String>,
    pub summary: Option<String>,
    pub work_history: Vec<LinkedInPosition>,
    pub education_history: Vec<LinkedInEducation>,
    pub skills: Vec<String>,
}

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let from = haystack.find(start)? + start.len();
    let remainder = haystack.get(from..)?;
    let to = remainder.find(end)?;
    Some(
        remainder[..to]
            .replace("&amp;", "&")
            .replace("&nbsp;", " ")
            .trim()
            .to_string(),
    )
}

fn collect_values(html: &str, start: &str, end: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut remainder = html;

    while let Some(idx) = remainder.find(start) {
        let after = &remainder[idx + start.len()..];
        let Some(end_idx) = after.find(end) else {
            break;
        };
        let value = after[..end_idx].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        remainder = &after[end_idx + end.len()..];
    }

    values
}

/// Parse the LinkedIn pipeline config.
#[must_use]
pub fn parse_pipeline_config(toml_str: &str) -> LinkedInPipelineConfig {
    let mut config = LinkedInPipelineConfig {
        name: String::new(),
        rate_limit_seconds: 0,
        requires_browser: true,
        wait_for: String::new(),
    };

    for line in toml_str.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "name" => config.name = value.to_string(),
                "rate_limit_seconds" => {
                    config.rate_limit_seconds = value.parse::<u64>().unwrap_or(0);
                }
                "requires_browser" => config.requires_browser = value == "true",
                "wait_for" => config.wait_for = value.to_string(),
                _ => {}
            }
        }
    }

    config
}

/// Parse mock or previously fetched LinkedIn public profile HTML.
#[must_use]
pub fn parse_public_profile_html(slug: &str, html: &str) -> Option<LinkedInProfile> {
    let name = extract_between(html, "data-profile-name=\"", "\"")?;
    let headline = extract_between(html, "data-profile-headline=\"", "\"");
    let current_company = extract_between(html, "data-profile-current-company=\"", "\"");
    let location = extract_between(html, "data-profile-location=\"", "\"");
    let summary = extract_between(html, "<section data-summary><p>", "</p>");

    let work_titles = collect_values(html, "data-role-title=\"", "\"");
    let work_companies = collect_values(html, "data-role-company=\"", "\"");
    let work_dates = collect_values(html, "data-role-dates=\"", "\"");
    let work_descriptions = collect_values(html, "data-role-description=\"", "\"");

    let work_history = work_titles
        .iter()
        .zip(work_companies.iter())
        .enumerate()
        .map(|(index, (title, company))| LinkedInPosition {
            title: title.clone(),
            company: company.clone(),
            dates: work_dates.get(index).cloned(),
            description: work_descriptions.get(index).cloned(),
        })
        .collect();

    let schools = collect_values(html, "data-edu-school=\"", "\"");
    let degrees = collect_values(html, "data-edu-degree=\"", "\"");
    let fields = collect_values(html, "data-edu-field=\"", "\"");
    let edu_dates = collect_values(html, "data-edu-dates=\"", "\"");

    let education_history = schools
        .iter()
        .enumerate()
        .map(|(index, school)| LinkedInEducation {
            school: school.clone(),
            degree: degrees.get(index).cloned(),
            field: fields.get(index).cloned(),
            dates: edu_dates.get(index).cloned(),
        })
        .collect();

    let skills = collect_values(html, "data-skill=\"", "\"");

    Some(LinkedInProfile {
        slug: slug.to_string(),
        name,
        headline,
        current_company,
        location,
        summary,
        work_history,
        education_history,
        skills,
    })
}

/// Persist a parsed LinkedIn public profile.
///
/// # Errors
///
/// Returns `Err` if the parsed profile cannot be serialized or written.
pub async fn save_public_profile(
    slug: &str,
    html: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let profile = parse_public_profile_html(slug, html).ok_or_else(|| {
        FetchError::Parse("could not parse LinkedIn public profile HTML".to_string())
    })?;

    let records = vec![serde_json::to_value(profile).map_err(|err| FetchError::Parse(err.to_string()))?];
    let output_path = output_dir.join("linkedin_public.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "linkedin_public".into(),
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
            <main
              data-profile-name="Jane Investigator"
              data-profile-headline="Open Source Intelligence Lead"
              data-profile-current-company="Grey Heron Analytics"
              data-profile-location="London, United Kingdom">
            </main>
            <section data-summary><p>Investigates sanctions evasion, shipping networks, and shell structures.</p></section>
            <div data-role-title="Founder"></div>
            <div data-role-company="Grey Heron Analytics"></div>
            <div data-role-dates="2021 - Present"></div>
            <div data-role-description="Built investigative workflows for corporate network analysis"></div>
            <div data-role-title="Researcher"></div>
            <div data-role-company="Civic Data Lab"></div>
            <div data-role-dates="2018 - 2021"></div>
            <div data-role-description="Tracked procurement anomalies"></div>
            <div data-edu-school="King's College London"></div>
            <div data-edu-degree="MSc"></div>
            <div data-edu-field="Digital Humanities"></div>
            <div data-edu-dates="2016 - 2017"></div>
            <div data-skill="Investigations"></div>
            <div data-skill="Sanctions Screening"></div>
            <div data-skill="Corporate Research"></div>
          </body>
        </html>
        "#
    }

    #[test]
    fn linkedin_public_pipeline_config_loads_without_browser_launch() {
        let config = parse_pipeline_config(PIPELINE_CONFIG);

        assert_eq!(config.name, "linkedin_public");
        assert_eq!(config.rate_limit_seconds, 30);
        assert!(config.requires_browser);
        assert_eq!(config.wait_for, "profile-content");
    }

    #[test]
    fn linkedin_public_extracts_core_profile_fields_from_mock_html() {
        let profile = parse_public_profile_html("jane-investigator", profile_fixture()).unwrap();

        assert_eq!(profile.name, "Jane Investigator");
        assert_eq!(profile.headline.as_deref(), Some("Open Source Intelligence Lead"));
        assert_eq!(profile.current_company.as_deref(), Some("Grey Heron Analytics"));
        assert_eq!(profile.location.as_deref(), Some("London, United Kingdom"));
    }

    #[test]
    fn linkedin_public_extracts_work_and_education_history() {
        let profile = parse_public_profile_html("jane-investigator", profile_fixture()).unwrap();

        assert_eq!(profile.work_history.len(), 2);
        assert_eq!(profile.work_history[0].title, "Founder");
        assert_eq!(profile.work_history[1].company, "Civic Data Lab");
        assert_eq!(profile.education_history.len(), 1);
        assert_eq!(profile.education_history[0].school, "King's College London");
        assert_eq!(profile.education_history[0].field.as_deref(), Some("Digital Humanities"));
    }
}