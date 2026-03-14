//! ICIJ Offshore Leaks — offshore entity/officer/address data.
//!
//! Source: <https://offshoreleaks.icij.org/> (bulk CSV download).
//! No pagination — single ZIP download.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const _BULK_URL: &str =
    "https://offshoreleaks-data.icij.org/offshoreleaks/csv/full-oldb.LATEST.zip";

/// Parse a CSV line into entity fields (simplified parser for ICIJ nodes).
pub fn parse_entity_csv_line(line: &str) -> Option<serde_json::Value> {
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 3 {
        return None;
    }
    Some(serde_json::json!({
        "node_id": fields[0].trim_matches('"'),
        "name": fields[1].trim_matches('"'),
        "jurisdiction": fields.get(2).unwrap_or(&"").trim_matches('"'),
        "source": "icij-leaks"
    }))
}

/// Fetch ICIJ offshore leaks data (entity search via API).
pub async fn fetch_entities(
    query: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get("https://offshoreleaks.icij.org/api/v1/search")
        .query(&[("q", query), ("e", "true")])
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
    let records = json
        .as_array()
        .cloned()
        .unwrap_or_default();

    let output_path = output_dir.join("icij_leaks.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "icij-leaks".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icij_csv_parser_extracts_node_fields() {
        let line = r#""10000001","Acme Offshore Ltd","BVI""#;
        let record = parse_entity_csv_line(line).unwrap();
        assert_eq!(record["node_id"], "10000001");
        assert_eq!(record["name"], "Acme Offshore Ltd");
        assert_eq!(record["jurisdiction"], "BVI");
    }
}
