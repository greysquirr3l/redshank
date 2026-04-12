//! UK HM Treasury (OFSI) — Office of Financial Sanctions Implementation consolidated list.
//!
//! Source: <https://ofsistorage.blob.core.windows.net/publishlive/ConList.csv>
//! No authentication required. Bulk CSV download.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const HMT_CSV_URL: &str = "https://ofsistorage.blob.core.windows.net/publishlive/ConList.csv";

/// A UK HM Treasury OFSI consolidated sanctions list entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HmtSanctionsEntry {
    /// Unique group ID for the entity (all aliases share the same group ID).
    pub group_id: String,
    /// Entity type (Individual or Entity).
    pub entity_type: String,
    /// Name 1 (primary name or last name).
    pub name1: String,
    /// Name 2 (first name or "N/A").
    pub name2: String,
    /// Name 3 (middle name or "N/A").
    pub name3: String,
    /// Name 4.
    pub name4: String,
    /// Name 5.
    pub name5: String,
    /// Name 6.
    pub name6: String,
    /// Description of the listing (occupation/role).
    pub title: String,
    /// Designation / regime name (e.g., Russia, ISIL, Taliban).
    pub regime: String,
    /// Listing date.
    pub listed_on: String,
    /// Last updated date.
    pub last_updated: String,
}

/// Fetch the UK HMT OFSI consolidated sanctions list (CSV).
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or returns a non-2xx status.
pub async fn fetch_hmt_sanctions(output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(HMT_CSV_URL).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let csv = resp.text().await?;
    let records = parse_hmt_csv(&csv);
    let serialized: Vec<serde_json::Value> = records
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let output_path = output_dir.join("uk_hmt_sanctions.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "uk_hmt_sanctions".into(),
        attribution: None,
    })
}

/// Parse the HMT OFSI CSV into a list of sanctions entries.
///
/// The CSV has a descriptive preamble block before the data header — this
/// function skips lines until it finds `"GroupID"` in the header row.
#[must_use]
pub fn parse_hmt_csv(csv: &str) -> Vec<HmtSanctionsEntry> {
    let mut lines = csv.lines();
    let mut header_idx: Option<usize> = None;
    let mut header: Vec<&str> = Vec::new();

    // Scan for the header row containing "GroupID"
    for (i, line) in csv.lines().enumerate() {
        if line.contains("GroupID") || line.contains("Group ID") {
            header = split_csv_line(line);
            header_idx = Some(i);
            break;
        }
    }

    let Some(skip_to) = header_idx else {
        return Vec::new();
    };

    // Skip lines up to and including the header
    for _ in 0..=skip_to {
        lines.next();
    }

    let col = |name: &str| -> Option<usize> {
        header
            .iter()
            .position(|h| h.trim_matches('"').eq_ignore_ascii_case(name))
    };

    let idx_group_id = col("GroupID").or_else(|| col("Group ID")).unwrap_or(0);
    let idx_entity_type = col("Entity_Type_Description")
        .or_else(|| col("Entity Type"))
        .unwrap_or(1);
    let idx_name1 = col("Name 1").or_else(|| col("Name1")).unwrap_or(2);
    let idx_name2 = col("Name 2").or_else(|| col("Name2")).unwrap_or(3);
    let idx_name3 = col("Name 3").or_else(|| col("Name3")).unwrap_or(4);
    let idx_name4 = col("Name 4").or_else(|| col("Name4")).unwrap_or(5);
    let idx_name5 = col("Name 5").or_else(|| col("Name5")).unwrap_or(6);
    let idx_name6 = col("Name 6").or_else(|| col("Name6")).unwrap_or(7);
    let idx_title = col("Title").unwrap_or(8);
    let idx_regime = col("Regime Name").or_else(|| col("Regime")).unwrap_or(9);
    let idx_listed_on = col("Listed On").or_else(|| col("ListedOn")).unwrap_or(10);
    let idx_last_updated = col("UK Sanctions List Date Designated")
        .or_else(|| col("Last Updated"))
        .unwrap_or(11);

    let get = |fields: &[&str], idx: usize| -> String {
        fields
            .get(idx)
            .map_or("", |s| s.trim_matches('"').trim())
            .to_string()
    };

    lines
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let fields: Vec<&str> = split_csv_line(line);
            HmtSanctionsEntry {
                group_id: get(&fields, idx_group_id),
                entity_type: get(&fields, idx_entity_type),
                name1: get(&fields, idx_name1),
                name2: get(&fields, idx_name2),
                name3: get(&fields, idx_name3),
                name4: get(&fields, idx_name4),
                name5: get(&fields, idx_name5),
                name6: get(&fields, idx_name6),
                title: get(&fields, idx_title),
                regime: get(&fields, idx_regime),
                listed_on: get(&fields, idx_listed_on),
                last_updated: get(&fields, idx_last_updated),
            }
        })
        .collect()
}

/// Minimal CSV line splitter that handles quoted fields with embedded commas.
fn split_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;
    let bytes = line.as_bytes();

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_quotes = !in_quotes,
            b',' if !in_quotes => {
                fields.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    fields.push(&line[start..]);
    fields
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const CSV_FIXTURE: &str = r#"HM Treasury Consolidated List of Financial Sanctions Targets
Generated: 01/01/2024
This document is not available in alternative formats
GroupID,Entity_Type_Description,Name 1,Name 2,Name 3,Name 4,Name 5,Name 6,Title,Date of Birth,Town of Birth,Country of Birth,Nationality,Passport Number,National Identification Number,Regime Name,Listed On,UK Sanctions List Date Designated,Other Information,UN Reference Number
12345,Individual,PUTIN,Vladimir,Vladimirovich,,,,"President of the Russian Federation",1952-10-07,Leningrad,Russia,Russian,P1234567890,,Russia,2022-02-28,2022-02-28,,
67890,Entity,SBERBANK OF RUSSIA,,,,,,Commercial bank,,,,,,Russia financial institution,Russia,2022-03-12,2022-03-12,State-owned bank,
"#;

    #[test]
    fn hmt_parses_csv_extracts_names_regime_listing_date() {
        let records = parse_hmt_csv(CSV_FIXTURE);
        assert_eq!(records.len(), 2);

        assert_eq!(records[0].group_id, "12345");
        assert_eq!(records[0].entity_type, "Individual");
        assert_eq!(records[0].name1, "PUTIN");
        assert_eq!(records[0].name2, "Vladimir");
        assert_eq!(records[0].regime, "Russia");
        assert_eq!(records[0].listed_on, "2022-02-28");
    }

    #[test]
    fn hmt_parses_entity_row() {
        let records = parse_hmt_csv(CSV_FIXTURE);

        assert_eq!(records[1].group_id, "67890");
        assert_eq!(records[1].entity_type, "Entity");
        assert_eq!(records[1].name1, "SBERBANK OF RUSSIA");
        assert_eq!(records[1].regime, "Russia");
    }

    #[test]
    fn hmt_handles_missing_header() {
        let records = parse_hmt_csv("no header here\nsome,random,data\n");
        assert!(records.is_empty());
    }

    #[test]
    fn hmt_handles_empty_csv() {
        let records = parse_hmt_csv("");
        assert!(records.is_empty());
    }
}
