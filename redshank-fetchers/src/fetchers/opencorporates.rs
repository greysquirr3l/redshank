//! `OpenCorporates` — Global corporate registry aggregator (200+ jurisdictions).
//!
//! API: `https://api.opencorporates.com/v0.4/companies/search`
//! Free tier: 500 requests/day. API key for higher volume.
//!
//! **Licence**: Data is published under the Open Database Licence (`ODbL` 1.0).
//! Any report or user-facing output that surfaces data from this source **must**
//! include a hyperlink reading "from `OpenCorporates`" linking to
//! `https://opencorporates.com` (or the canonical entity URL returned in the API
//! response). The attribution must be at least 70 % of the largest related font
//! and never smaller than 7 px. See <https://opencorporates.com/terms-of-use-2/>.

use crate::domain::{Attribution, FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://api.opencorporates.com/v0.4";

/// Returns the `ODbL` attribution that **must** accompany any output derived from
/// `OpenCorporates` data.
///
/// Attach the result to every [`FetchOutput`] and render it in all reports / UI.
/// The attribution text must hyperlink to `url`, be at least `min_font_size_px` pixels,
/// and never be smaller than 70 % of the largest related font on the page.
pub fn attribution() -> Attribution {
    Attribution {
        source: "OpenCorporates".into(),
        text: "from OpenCorporates".into(),
        url: "https://opencorporates.com".into(),
        min_font_size_px: 7,
        licence: "ODbL-1.0".into(),
    }
}

/// Search `OpenCorporates` for companies matching `name`, optionally filtered by
/// `jurisdiction_code` (e.g. `us_de` for Delaware, `gb` for UK).
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_companies(
    name: &str,
    jurisdiction_code: Option<&str>,
    api_token: Option<&str>,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 1..=max {
        let mut req = client.get(format!("{API_BASE}/companies/search")).query(&[
            ("q", name),
            ("page", &page.to_string()),
            ("per_page", "100"),
        ]);

        if let Some(jc) = jurisdiction_code {
            req = req.query(&[("jurisdiction_code", jc)]);
        }
        if let Some(token) = api_token {
            req = req.query(&[("api_token", token)]);
        }

        let resp = req.send().await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(FetchError::ApiError {
                status: status.as_u16(),
                body: text,
            });
        }

        let json: serde_json::Value = resp.json().await?;
        let companies = json
            .get("results")
            .and_then(|r| r.get("companies"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if companies.is_empty() {
            break;
        }
        all_records.extend(companies);

        let total_pages = u32::try_from(
            json.get("results")
                .and_then(|r| r.get("total_pages"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1),
        )
        .unwrap_or(u32::MAX);

        if page >= total_pages {
            break;
        }
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("opencorporates.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "opencorporates".into(),
        // ODbL-1.0 requires attribution in every report / user-facing output.
        // See: https://opencorporates.com/terms-of-use-2/
        attribution: Some(attribution()),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    #[test]
    fn opencorporates_parses_company_search_response() {
        let mock = serde_json::json!({
            "results": {
                "companies": [
                    {
                        "company": {
                            "company_number": "1234567",
                            "name": "ACME SHELL CORP LLC",
                            "jurisdiction_code": "us_de",
                            "registered_address_in_full": "1209 Orange St, Wilmington, DE 19801",
                            "incorporation_date": "2020-01-15",
                            "current_status": "Good Standing"
                        }
                    }
                ],
                "total_pages": 1,
                "total_count": 1,
                "page": 1
            }
        });
        let companies = mock["results"]["companies"].as_array().unwrap();
        assert_eq!(companies.len(), 1);
        let co = &companies[0]["company"];
        assert_eq!(co["company_number"], "1234567");
        assert_eq!(co["jurisdiction_code"], "us_de");
        assert!(
            co["registered_address_in_full"]
                .as_str()
                .unwrap()
                .contains("Wilmington")
        );
    }
}
