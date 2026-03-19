//! EPA ECHO — Enforcement and Compliance History Online.
//!
//! API: <https://echo.epa.gov/rest/services/>
//! Two-step: query → QID token → paginated results.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://echodata.epa.gov/echo/echo_rest_services";

/// Fetch EPA ECHO facility enforcement data.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_facilities(
    query: &str,
    state: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    // Step 1: Initial query to obtain QID.
    let mut params = vec![("p_fn", query.to_string()), ("output", "JSON".to_string())];
    if let Some(st) = state {
        params.push(("p_st", st.to_string()));
    }

    let resp = client
        .get(format!("{API_BASE}.get_facilities"))
        .query(&params)
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
    let qid = json
        .pointer("/Results/QueryID")
        .and_then(|v| v.as_str())
        .ok_or_else(|| FetchError::Parse("no QueryID in response".into()))?
        .to_string();

    // Step 2: Fetch pages using QID.
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let resp = client
            .get(format!("{API_BASE}.get_qid"))
            .query(&[
                ("qid", qid.as_str()),
                ("pageno", &page.to_string()),
                ("output", "JSON"),
            ])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            break;
        }

        let json: serde_json::Value = resp.json().await?;
        let facilities = json
            .pointer("/Results/Facilities")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if facilities.is_empty() {
            break;
        }
        all_records.extend(facilities);

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("epa_echo.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "epa-echo".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[test]
    fn epa_echo_parses_qid_response() {
        let mock = serde_json::json!({
            "Results": {
                "QueryID": "ABC123",
                "NumResults": "5"
            }
        });
        let qid = mock
            .pointer("/Results/QueryID")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(qid, "ABC123");
    }
}
