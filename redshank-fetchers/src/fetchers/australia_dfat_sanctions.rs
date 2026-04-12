//! Australia DFAT — Department of Foreign Affairs and Trade consolidated sanctions list.
//!
//! Source: <https://www.dfat.gov.au/international-relations/security/sanctions/consolidated-list>
//! The page links to a CSV/XML bulk download. No authentication required.
//! This fetcher parses the CSV format used by DFAT.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

/// DFAT consolidated list — direct CSV download URL.
const DFAT_CSV_URL: &str =
    "https://www.dfat.gov.au/sites/default/files/regulation8_consolidated.csv";

/// An Australia DFAT sanctions list entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DfatSanctionsEntry {
    /// Name type (Individual or Entity).
    pub name_type: String,
    /// Primary name.
    pub name: String,
    /// Alias names, if any.
    pub aliases: Vec<String>,
    /// Date of birth (for individuals).
    pub date_of_birth: Option<String>,
    /// Place of birth (for individuals).
    pub place_of_birth: Option<String>,
    /// Citizenship/nationality.
    pub citizenship: Option<String>,
    /// Address.
    pub address: Option<String>,
    /// Regime or sanctions program.
    pub listing_information: String,
    /// Date of designation.
    pub control_date: Option<String>,
}

/// Fetch the Australia DFAT consolidated sanctions list (CSV).
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or returns a non-2xx status.
pub async fn fetch_dfat_sanctions(output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(DFAT_CSV_URL).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let csv = resp.text().await?;
    let records = parse_dfat_csv(&csv);
    let serialized: Vec<serde_json::Value> = records
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let output_path = output_dir.join("australia_dfat_sanctions.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "australia_dfat_sanctions".into(),
        attribution: None,
    })
}

/// Parse the DFAT consolidated sanctions CSV.
///
/// Expected columns (approximately): Type, Name, DOB, Place of Birth,
/// Citizenship, Address, Additional Information, Control Date, Aliases…
#[must_use]
pub fn parse_dfat_csv(csv: &str) -> Vec<DfatSanctionsEntry> {
    let mut lines = csv.lines().peekable();
    let mut header: Vec<String> = Vec::new();

    // Find the header row
    for line in csv.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("name") && (lower.contains("type") || lower.contains("dob")) {
            header = split_csv_line(line)
                .into_iter()
                .map(|s| s.trim_matches('"').trim().to_ascii_lowercase())
                .collect();
            break;
        }
    }

    if header.is_empty() {
        return Vec::new();
    }

    // Skip past the header row
    let header_content: String = header.join(",").to_ascii_lowercase();
    while let Some(l) = lines.peek() {
        let lower = l.to_ascii_lowercase();
        if lower.contains(&header_content[..header_content.len().min(30)]) {
            lines.next();
            break;
        }
        lines.next();
    }

    let col = |name: &str| -> usize {
        header
            .iter()
            .position(|h| h.contains(name))
            .unwrap_or(usize::MAX)
    };

    let idx_type = col("type");
    let idx_name = col("name");
    let idx_dob = col("dob").min(col("date of birth")).min(col("birth"));
    let idx_birth_place = col("place of birth").min(col("place_of_birth"));
    let idx_citizenship = col("citizenship").min(col("nationality"));
    let idx_address = col("address");
    let idx_listing = col("listing").min(col("information")).min(col("program"));
    let idx_control_date = col("control date")
        .min(col("control_date"))
        .min(col("designation"));

    let get = |fields: &[String], idx: usize| -> Option<String> {
        if idx == usize::MAX {
            return None;
        }
        fields.get(idx).and_then(|s| {
            let v = s.trim_matches('"').trim().to_string();
            if v.is_empty() { None } else { Some(v) }
        })
    };

    lines
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let fields: Vec<String> = split_csv_line(line)
                .into_iter()
                .map(|s| s.trim_matches('"').trim().to_string())
                .collect();

            let name_type = get(&fields, idx_type).unwrap_or_default();
            let name = get(&fields, idx_name).unwrap_or_default();

            // Collect all non-empty fields beyond standard columns as potential aliases
            // DFAT sometimes has Alias 1, Alias 2 columns at the end
            let aliases: Vec<String> = fields
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    !matches!(*i, _ if *i == idx_type || *i == idx_name
                        || *i == idx_dob || *i == idx_birth_place
                        || *i == idx_citizenship || *i == idx_address
                        || *i == idx_listing || *i == idx_control_date)
                })
                .filter_map(|(_, v)| {
                    let s = v.trim_matches('"').trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                })
                .collect();

            DfatSanctionsEntry {
                name_type,
                name,
                aliases,
                date_of_birth: get(&fields, idx_dob),
                place_of_birth: get(&fields, idx_birth_place),
                citizenship: get(&fields, idx_citizenship),
                address: get(&fields, idx_address),
                listing_information: get(&fields, idx_listing).unwrap_or_default(),
                control_date: get(&fields, idx_control_date),
            }
        })
        .collect()
}

fn split_csv_line(line: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;

    for (i, b) in line.bytes().enumerate() {
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

impl DfatSanctionsEntry {
    /// Returns the entity type string (alias for `name_type`).
    #[must_use]
    pub fn entity_type(&self) -> &str {
        &self.name_type
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const CSV_FIXTURE: &str = r#"Type,Name,DOB,Place of Birth,Citizenship,Address,Listing Information,Control Date,Alias 1
Individual,"NGUYEN, Van Thanh","1975-05-12","Ho Chi Minh City","Vietnamese","Unknown","Myanmar autonomous sanctions",2021-02-08,"NGUYEN THANH VAN"
Entity,"Global Resources Pte Ltd",,,,,,"Russia autonomous sanctions",2022-03-10,
"#;

    #[test]
    fn dfat_parses_csv_extracts_type_name_and_listing() {
        let records = parse_dfat_csv(CSV_FIXTURE);
        assert_eq!(records.len(), 2);

        assert_eq!(records[0].name_type, "Individual");
        assert_eq!(records[0].name, "NGUYEN, Van Thanh");
        assert_eq!(records[0].date_of_birth.as_deref(), Some("1975-05-12"));
        assert_eq!(records[0].citizenship.as_deref(), Some("Vietnamese"));
    }

    #[test]
    fn dfat_parses_entity_row() {
        let records = parse_dfat_csv(CSV_FIXTURE);
        assert_eq!(records[1].entity_type(), "Entity");
        assert_eq!(records[1].name, "Global Resources Pte Ltd");
    }

    #[test]
    fn dfat_handles_empty_csv() {
        let records = parse_dfat_csv("");
        assert!(records.is_empty());
    }
}
