//! ClinicalTrials.gov parser and fetcher helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://clinicaltrials.gov/api/v2/studies";

/// A normalized ClinicalTrials.gov study record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ClinicalTrialRecord {
    pub nct_id: String,
    pub title: String,
    pub overall_status: Option<String>,
    pub sponsor: Option<String>,
    pub principal_investigator: Option<String>,
    pub collaborators: Vec<String>,
    pub conditions: Vec<String>,
    pub interventions: Vec<String>,
    pub funding_source: Option<String>,
}

fn optional_string(json: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = json;
    for key in path {
        current = current.get(*key)?;
    }
    current
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

/// Parse a ClinicalTrials.gov studies response.
#[must_use]
pub fn parse_trials(json: &serde_json::Value) -> Vec<ClinicalTrialRecord> {
    json.get("studies")
        .and_then(serde_json::Value::as_array)
        .map(|studies| {
            studies
                .iter()
                .filter_map(|study| {
                    let protocol = study.get("protocolSection")?;
                    let identification = protocol.get("identificationModule")?;
                    let status = protocol.get("statusModule");
                    let sponsor = protocol.get("sponsorCollaboratorsModule");
                    let conditions = protocol.get("conditionsModule");
                    let arms = protocol.get("armsInterventionsModule");
                    let contacts = protocol.get("contactsLocationsModule");

                    let nct_id = identification
                        .get("nctId")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();
                    let title = identification
                        .get("briefTitle")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();

                    let principal_investigator = contacts
                        .and_then(|module| module.get("overallOfficials"))
                        .and_then(serde_json::Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(|official| official.get("name"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string);

                    let collaborators = sponsor
                        .and_then(|module| module.get("collaborators"))
                        .and_then(serde_json::Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    item.get("name").and_then(serde_json::Value::as_str)
                                })
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default();

                    let interventions = arms
                        .and_then(|module| module.get("interventions"))
                        .and_then(serde_json::Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    item.get("name").and_then(serde_json::Value::as_str)
                                })
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default();

                    Some(ClinicalTrialRecord {
                        nct_id,
                        title,
                        overall_status: status
                            .and_then(|module| module.get("overallStatus"))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                        sponsor: sponsor
                            .and_then(|module| module.get("leadSponsor"))
                            .and_then(|lead| lead.get("name"))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                        principal_investigator,
                        collaborators,
                        conditions: conditions
                            .and_then(|module| module.get("conditions"))
                            .and_then(serde_json::Value::as_array)
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(serde_json::Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default(),
                        interventions,
                        funding_source: optional_string(
                            protocol,
                            &[
                                "sponsorCollaboratorsModule",
                                "responsibleParty",
                                "investigatorAffiliation",
                            ],
                        )
                        .or_else(|| {
                            sponsor
                                .and_then(|module| module.get("leadSponsor"))
                                .and_then(|lead| lead.get("class"))
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_string)
                        }),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch trials matching a query term.
///
/// # Errors
///
/// Returns `Err` if the request fails or the server returns a non-success status.
pub async fn fetch_trials(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get(API_BASE)
        .query(&[("query.term", query), ("pageSize", "100")])
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
    let records = parse_trials(&json)
        .into_iter()
        .map(|record| {
            serde_json::to_value(record).map_err(|err| FetchError::Parse(err.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    rate_limit_delay(rate_limit_ms).await;

    let output_path = output_dir.join("clinical_trials.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "clinical-trials".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn trials_fixture() -> serde_json::Value {
        serde_json::json!({
            "studies": [
                {
                    "protocolSection": {
                        "identificationModule": {
                            "nctId": "NCT01234567",
                            "briefTitle": "Trial of CardioX in Heart Failure"
                        },
                        "statusModule": {
                            "overallStatus": "COMPLETED"
                        },
                        "sponsorCollaboratorsModule": {
                            "leadSponsor": {
                                "name": "PharmaCo",
                                "class": "INDUSTRY"
                            },
                            "collaborators": [
                                {"name": "NIH"},
                                {"name": "Austin Heart Institute"}
                            ],
                            "responsibleParty": {
                                "investigatorAffiliation": "Industry"
                            }
                        },
                        "conditionsModule": {
                            "conditions": ["Heart Failure", "Cardiomyopathy"]
                        },
                        "armsInterventionsModule": {
                            "interventions": [
                                {"name": "CardioX"},
                                {"name": "Standard of Care"}
                            ]
                        },
                        "contactsLocationsModule": {
                            "overallOfficials": [
                                {"name": "Dr. Alice Carter", "role": "PRINCIPAL_INVESTIGATOR"}
                            ]
                        }
                    }
                }
            ]
        })
    }

    #[test]
    fn clinical_trials_parses_trial_search_fixture() {
        let records = parse_trials(&trials_fixture());

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].nct_id, "NCT01234567");
        assert_eq!(records[0].title, "Trial of CardioX in Heart Failure");
    }

    #[test]
    fn clinical_trials_extracts_sponsor_pi_conditions_and_interventions() {
        let records = parse_trials(&trials_fixture());

        assert_eq!(records[0].sponsor.as_deref(), Some("PharmaCo"));
        assert_eq!(
            records[0].principal_investigator.as_deref(),
            Some("Dr. Alice Carter")
        );
        assert!(records[0].conditions.contains(&"Heart Failure".to_string()));
        assert!(records[0].interventions.contains(&"CardioX".to_string()));
    }

    #[test]
    fn clinical_trials_extracts_funding_source_and_collaborators() {
        let records = parse_trials(&trials_fixture());

        assert_eq!(records[0].funding_source.as_deref(), Some("Industry"));
        assert!(records[0].collaborators.contains(&"NIH".to_string()));
        assert!(
            records[0]
                .collaborators
                .contains(&"Austin Heart Institute".to_string())
        );
    }
}
