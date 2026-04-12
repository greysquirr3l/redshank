//! IRS exemption application parser and document fetch helper.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// A planned activity described in a Form 1023 or 1024-A filing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PlannedActivity {
    /// Activity title or label.
    pub heading: Option<String>,
    /// Narrative description of the activity.
    pub description: String,
}

/// A normalized IRS exemption application record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Irs1023Application {
    /// Organization EIN if available.
    pub ein: Option<String>,
    /// IRS form type, such as 1023 or 1024-A.
    pub form_type: Option<String>,
    /// Organization name.
    pub organization_name: Option<String>,
    /// Mission or exempt purpose statement.
    pub mission: Option<String>,
    /// Organizational history or narrative.
    pub organizational_narrative: Option<String>,
    /// Planned activities disclosed in the application.
    pub planned_activities: Vec<PlannedActivity>,
    /// Board members named in the filing.
    pub board_members: Vec<String>,
    /// Financial projection summary text.
    pub financial_projection: Option<String>,
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

/// Parse an IRS exemption application fixture or OCR output.
#[must_use]
pub fn parse_irs_1023_application(document: &str) -> Option<Irs1023Application> {
    let organization_name = extract_between(document, "data-org-name=\"", "\"")
        .or_else(|| extract_between(document, "Organization Name:", "\n"));

    let planned_titles = collect_attr_values(document, "data-activity-heading");
    let planned_descriptions = collect_attr_values(document, "data-activity-description");
    let planned_activities = planned_descriptions
        .iter()
        .enumerate()
        .map(|(index, description)| PlannedActivity {
            heading: planned_titles.get(index).cloned(),
            description: description.clone(),
        })
        .collect::<Vec<_>>();

    Some(Irs1023Application {
        ein: extract_between(document, "data-ein=\"", "\"")
            .or_else(|| extract_between(document, "EIN:", "\n")),
        form_type: extract_between(document, "data-form-type=\"", "\"")
            .or_else(|| extract_between(document, "Form Type:", "\n")),
        organization_name,
        mission: extract_between(document, "data-mission=\"", "\"")
            .or_else(|| extract_between(document, "Mission:", "\n")),
        organizational_narrative: extract_between(
            document,
            "<section data-org-narrative><p>",
            "</p>",
        )
        .or_else(|| extract_between(document, "Organizational Narrative:", "\n\n")),
        planned_activities,
        board_members: collect_attr_values(document, "data-board-member"),
        financial_projection: extract_between(document, "data-financial-projection=\"", "\"")
            .or_else(|| extract_between(document, "Financial Projection:", "\n")),
    })
}

/// Fetch an IRS exemption application document and persist a parsed record.
///
/// # Errors
///
/// Returns `Err` if the document request fails or parsing/writing fails.
pub async fn fetch_irs_1023_document(
    document_url: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(document_url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let body = resp.text().await?;
    let application = parse_irs_1023_application(&body)
        .ok_or_else(|| FetchError::Parse("could not parse IRS exemption application".to_string()))?;
    let records = vec![serde_json::to_value(application)
        .map_err(|err| FetchError::Parse(err.to_string()))?];
    let output_path = output_dir.join("irs_1023.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "irs-1023".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn application_fixture() -> &'static str {
        r#"
        <html>
          <body>
            <main data-org-name="Community Justice Lab" data-ein="521234567" data-form-type="1023" data-mission="Advance community-led accountability efforts."></main>
            <section data-org-narrative><p>Founded by former public defenders to build community-based legal education clinics.</p></section>
            <div data-activity-heading="Legal Clinics" data-activity-description="Operate free legal rights workshops and intake clinics for tenants and workers."></div>
            <div data-activity-heading="Research" data-activity-description="Publish reports on court debt and pretrial detention trends."></div>
            <div data-board-member="Nina Alvarez"></div>
            <div data-board-member="Owen Brooks"></div>
            <div data-financial-projection="Projected year-one revenue of $450,000 with $380,000 in program expenses."></div>
          </body>
        </html>
        "#
    }

    #[test]
    fn irs_1023_parses_exemption_application_fixture() {
        let application = parse_irs_1023_application(application_fixture()).unwrap();

        assert_eq!(application.organization_name.as_deref(), Some("Community Justice Lab"));
        assert_eq!(application.ein.as_deref(), Some("521234567"));
        assert_eq!(application.form_type.as_deref(), Some("1023"));
        assert_eq!(application.board_members.len(), 2);
    }

    #[test]
    fn irs_1023_extracts_narrative_and_planned_activities() {
        let application = parse_irs_1023_application(application_fixture()).unwrap();

        assert!(application
            .organizational_narrative
            .as_deref()
            .unwrap()
            .contains("public defenders"));
        assert_eq!(application.planned_activities.len(), 2);
        assert_eq!(
            application.planned_activities[0].heading.as_deref(),
            Some("Legal Clinics")
        );
        assert!(application.planned_activities[1]
            .description
            .contains("court debt"));
    }
}