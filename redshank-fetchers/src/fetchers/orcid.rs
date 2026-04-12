//! ORCID Registry — persistent researcher identity, employment history, and works.
//!
//! Source: <https://pub.orcid.org/v3.0/> (public API, no auth for public records)
//! ORCID is the persistent digital identifier for researchers — like an SSN for academia.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://pub.orcid.org/v3.0";

/// A researcher profile from ORCID.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrcidProfile {
    /// ORCID iD (e.g. "0000-0002-1234-5678").
    pub orcid_id: String,
    /// Researcher name.
    pub name: Option<String>,
    /// Biography / personal statement.
    pub biography: Option<String>,
    /// Employment history (most recent first).
    pub employment: Vec<OrcidEmployment>,
    /// Educational history.
    pub education: Vec<OrcidEducation>,
    /// Published works (papers, datasets, etc.).
    pub works: Vec<OrcidWork>,
    /// Keywords self-described by the researcher.
    pub keywords: Vec<String>,
    /// External websites linked from the profile.
    pub websites: Vec<String>,
}

/// An employment record from an ORCID profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrcidEmployment {
    /// Employing organization.
    pub organization: String,
    /// Role / title.
    pub role: Option<String>,
    /// Start date (YYYY or YYYY-MM).
    pub start_date: Option<String>,
    /// End date (YYYY or YYYY-MM), `None` if current.
    pub end_date: Option<String>,
    /// Department.
    pub department: Option<String>,
}

/// An education record from an ORCID profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrcidEducation {
    /// Institution name.
    pub institution: String,
    /// Degree or qualification.
    pub degree: Option<String>,
    /// Field of study.
    pub field: Option<String>,
    /// Start date.
    pub start_date: Option<String>,
    /// End date.
    pub end_date: Option<String>,
}

/// A publication or work from an ORCID profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrcidWork {
    /// Work title.
    pub title: String,
    /// Work type ("journal-article", "dataset", "book", etc.).
    pub work_type: Option<String>,
    /// Publication year.
    pub year: Option<u32>,
    /// DOI if available.
    pub doi: Option<String>,
    /// Journal or publication venue.
    pub journal: Option<String>,
}

/// Parse an ORCID person record from the `/v3.0/{orcid}/record` endpoint.
///
/// The ORCID public API returns a nested XML-converted-to-JSON structure.
/// This parser handles both the full record shape and individual section shapes.
#[must_use]
pub fn parse_orcid_record(json: &serde_json::Value) -> Option<OrcidProfile> {
    // Navigate person sub-object (full record has this wrapper)
    let record = json.get("record").unwrap_or(json);
    let person = record.get("person").unwrap_or(record);

    // Extract ORCID ID from multiple possible paths
    let orcid_id = record
        .get("orcid-identifier")
        .and_then(|o| o.get("path"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| person.get("orcid-id").and_then(serde_json::Value::as_str))
        .map(String::from)?;

    // Name
    let name = person.get("name").and_then(|n| {
        let given = n
            .get("given-names")
            .and_then(|v| v.get("value"))
            .and_then(serde_json::Value::as_str);
        let family = n
            .get("family-name")
            .and_then(|v| v.get("value"))
            .and_then(serde_json::Value::as_str);
        match (given, family) {
            (Some(g), Some(f)) => Some(format!("{g} {f}")),
            (Some(g), None) => Some(g.to_string()),
            (None, Some(f)) => Some(f.to_string()),
            (None, None) => None,
        }
    });

    // Biography
    let biography = person
        .get("biography")
        .and_then(|b| b.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    // Keywords
    let keywords = person
        .get("keywords")
        .and_then(|kw| kw.get("keyword"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|k| {
                    k.get("content")
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    // Websites
    let websites = person
        .get("researcher-urls")
        .and_then(|ru| ru.get("researcher-url"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|w| {
                    w.get("url")
                        .and_then(|u| u.get("value"))
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default();

    // Employment
    let employment = record
        .get("activities-summary")
        .and_then(|a| a.get("employments"))
        .and_then(|e| e.get("affiliation-group"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_employment).collect())
        .unwrap_or_default();

    // Education
    let education = record
        .get("activities-summary")
        .and_then(|a| a.get("educations"))
        .and_then(|e| e.get("affiliation-group"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_education).collect())
        .unwrap_or_default();

    // Works
    let works = record
        .get("activities-summary")
        .and_then(|a| a.get("works"))
        .and_then(|w| w.get("group"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_work).collect())
        .unwrap_or_default();

    Some(OrcidProfile {
        orcid_id,
        name,
        biography,
        employment,
        education,
        works,
        keywords,
        websites,
    })
}

fn parse_date(date_obj: Option<&serde_json::Value>) -> Option<String> {
    let d = date_obj?;
    let year = d
        .get("year")
        .and_then(|y| y.get("value"))
        .and_then(serde_json::Value::as_str)?;
    let month = d
        .get("month")
        .and_then(|m| m.get("value"))
        .and_then(serde_json::Value::as_str);
    Some(month.map_or_else(|| year.to_string(), |m| format!("{year}-{m}")))
}

fn parse_employment(group: &serde_json::Value) -> Option<OrcidEmployment> {
    let summary = group
        .get("summaries")
        .and_then(serde_json::Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|s| s.get("employment-summary"))
        .or_else(|| group.get("employment-summary"))?;

    let organization = summary
        .get("organization")
        .and_then(|o| o.get("name"))
        .and_then(serde_json::Value::as_str)?
        .to_string();

    Some(OrcidEmployment {
        organization,
        role: summary
            .get("role-title")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        start_date: parse_date(summary.get("start-date")),
        end_date: parse_date(summary.get("end-date")),
        department: summary
            .get("department-name")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

fn parse_education(group: &serde_json::Value) -> Option<OrcidEducation> {
    let summary = group
        .get("summaries")
        .and_then(serde_json::Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|s| s.get("education-summary"))
        .or_else(|| group.get("education-summary"))?;

    let institution = summary
        .get("organization")
        .and_then(|o| o.get("name"))
        .and_then(serde_json::Value::as_str)?
        .to_string();

    Some(OrcidEducation {
        institution,
        degree: summary
            .get("role-title")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        field: summary
            .get("department-name")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        start_date: parse_date(summary.get("start-date")),
        end_date: parse_date(summary.get("end-date")),
    })
}

fn parse_work(group: &serde_json::Value) -> Option<OrcidWork> {
    let summary = group
        .get("work-summary")
        .and_then(serde_json::Value::as_array)
        .and_then(|arr| arr.first())
        .or_else(|| group.get("work-summary"))?;

    let title = summary
        .get("title")
        .and_then(|t| t.get("title"))
        .and_then(|t| t.get("value"))
        .and_then(serde_json::Value::as_str)?
        .to_string();

    let doi = summary
        .get("external-ids")
        .and_then(|e| e.get("external-id"))
        .and_then(serde_json::Value::as_array)
        .and_then(|arr| {
            arr.iter().find(|id| {
                id.get("external-id-type")
                    .and_then(serde_json::Value::as_str)
                    == Some("doi")
            })
        })
        .and_then(|id| id.get("external-id-value"))
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    Some(OrcidWork {
        title,
        work_type: summary
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        year: summary
            .get("publication-date")
            .and_then(|d| d.get("year"))
            .and_then(|y| y.get("value"))
            .and_then(serde_json::Value::as_str)
            .and_then(|s| s.parse().ok()),
        doi,
        journal: summary
            .get("journal-title")
            .and_then(|j| j.get("value"))
            .and_then(serde_json::Value::as_str)
            .map(String::from),
    })
}

/// Fetch an ORCID researcher profile by ORCID iD.
///
/// # Errors
///
/// Returns `Err` if the request fails or the profile is not public.
pub async fn fetch_orcid_profile(
    orcid_id: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    rate_limit_delay(rate_limit_ms).await;

    let resp = client
        .get(format!("{API_BASE}/{orcid_id}/record"))
        .header("accept", "application/json")
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
    let profile = parse_orcid_record(&json)
        .ok_or_else(|| FetchError::Parse("ORCID record missing required fields".to_string()))?;

    let serialized =
        serde_json::to_value(&profile).map_err(|e| FetchError::Parse(e.to_string()))?;

    let output_path = output_dir.join(format!("orcid_{}.ndjson", orcid_id.replace('-', "_")));
    let count = write_ndjson(&output_path, &[serialized])?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "orcid".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn orcid_fixture() -> serde_json::Value {
        serde_json::json!({
            "record": {
                "orcid-identifier": {"path": "0000-0002-1234-5678"},
                "person": {
                    "name": {
                        "given-names": {"value": "Jane"},
                        "family-name": {"value": "Researcher"}
                    },
                    "biography": {"content": "Professor of CS at MIT"},
                    "keywords": {"keyword": [{"content": "machine learning"}, {"content": "NLP"}]},
                    "researcher-urls": {"researcher-url": [{"url": {"value": "https://jresearcher.mit.edu"}}]}
                },
                "activities-summary": {
                    "employments": {
                        "affiliation-group": [
                            {
                                "summaries": [
                                    {
                                        "employment-summary": {
                                            "organization": {"name": "MIT"},
                                            "role-title": "Professor",
                                            "department-name": "CSAIL",
                                            "start-date": {"year": {"value": "2015"}, "month": {"value": "09"}},
                                            "end-date": null
                                        }
                                    }
                                ]
                            }
                        ]
                    },
                    "educations": {
                        "affiliation-group": [
                            {
                                "summaries": [
                                    {
                                        "education-summary": {
                                            "organization": {"name": "Stanford University"},
                                            "role-title": "PhD",
                                            "department-name": "Computer Science",
                                            "start-date": {"year": {"value": "2009"}},
                                            "end-date": {"year": {"value": "2014"}}
                                        }
                                    }
                                ]
                            }
                        ]
                    },
                    "works": {
                        "group": [
                            {
                                "work-summary": [
                                    {
                                        "title": {"title": {"value": "Neural Networks for Text Analysis"}},
                                        "type": "journal-article",
                                        "publication-date": {"year": {"value": "2022"}},
                                        "journal-title": {"value": "Nature Machine Intelligence"},
                                        "external-ids": {
                                            "external-id": [
                                                {"external-id-type": "doi", "external-id-value": "10.1038/s42256-022-00001-x"}
                                            ]
                                        }
                                    }
                                ]
                            }
                        ]
                    }
                }
            }
        })
    }

    #[test]
    fn orcid_parses_researcher_profile_fixture_extracts_employment_history_and_works() {
        let json = orcid_fixture();
        let profile = parse_orcid_record(&json).unwrap();

        assert_eq!(profile.orcid_id, "0000-0002-1234-5678");
        assert_eq!(profile.name.as_deref(), Some("Jane Researcher"));
        assert_eq!(profile.employment.len(), 1);
        assert_eq!(profile.employment[0].organization, "MIT");
        assert_eq!(profile.employment[0].role.as_deref(), Some("Professor"));
        assert_eq!(profile.employment[0].start_date.as_deref(), Some("2015-09"));
        assert_eq!(profile.works.len(), 1);
        assert_eq!(profile.works[0].title, "Neural Networks for Text Analysis");
        assert_eq!(
            profile.works[0].doi.as_deref(),
            Some("10.1038/s42256-022-00001-x")
        );
    }

    #[test]
    fn orcid_extracts_education_keywords_and_websites() {
        let json = orcid_fixture();
        let profile = parse_orcid_record(&json).unwrap();

        assert_eq!(profile.education.len(), 1);
        assert_eq!(profile.education[0].institution, "Stanford University");
        assert_eq!(profile.education[0].degree.as_deref(), Some("PhD"));
        assert_eq!(profile.keywords, vec!["machine learning", "NLP"]);
        assert!(profile.websites[0].contains("jresearcher.mit.edu"));
    }
}
