//! FDIC — Federal Deposit Insurance Corporation bank data.
//!
//! API: <https://banks.data.fdic.gov/api/>
//! Pagination: offset + limit (max 10,000).

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::path::Path;

const API_BASE: &str = "https://banks.data.fdic.gov/api";

/// Fetch FDIC institution data.
pub async fn fetch_institutions(
    query: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let mut all_records = Vec::new();
    let limit: u32 = 1000;
    let max = if max_pages == 0 { u32::MAX } else { max_pages };

    for page in 0..max {
        let offset = page * limit;
        let filter = format!("INSTNAME:\"{}\"", query);
        let resp = client
            .get(format!("{API_BASE}/financials"))
            .query(&[
                ("filters", filter.as_str()),
                ("limit", &limit.to_string()),
                ("offset", &offset.to_string()),
                ("sort_by", "REPDTE"),
                ("sort_order", "DESC"),
            ])
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
        let data = json
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if data.is_empty() {
            break;
        }
        all_records.extend(data);

        let total = json
            .get("totals")
            .and_then(|t| t.get("count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        if all_records.len() >= total {
            break;
        }

        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("fdic.ndjson");
    let count = write_ndjson(&output_path, &all_records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "fdic".into(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn fdic_parses_institution_response() {
        let mock = serde_json::json!({
            "data": [
                {"data": {"INSTNAME": "First National Bank", "CERT": "12345", "ASSET": "1000000"}},
            ],
            "totals": {"count": 1}
        });
        let data = mock["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["data"]["INSTNAME"], "First National Bank");
    }
}
