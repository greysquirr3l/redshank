//! FAA Civil Aviation Registry — aircraft registration (`N-Number` database).
//!
//! Source: <https://registry.faa.gov/database/ReleasableAircraft.zip>
//! Bulk download (updated monthly). No authentication required.
//! Parses MASTER.txt (registration records) from the ZIP.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const BULK_ZIP_URL: &str = "https://registry.faa.gov/database/ReleasableAircraft.zip";

/// An FAA aircraft registration record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AircraftRegistration {
    /// `N-Number` (tail number), e.g., "N12345".
    pub n_number: String,
    /// Aircraft serial number.
    pub serial_number: String,
    /// Aircraft make/manufacturer code.
    pub mfr_mdl_code: String,
    /// Registrant type (Corporation, Individual, Government, Partnership, etc.).
    pub registrant_type: String,
    /// Registrant name.
    pub registrant_name: String,
    /// Street address line 1.
    pub address_street: String,
    /// City.
    pub address_city: String,
    /// State abbreviation.
    pub address_state: String,
    /// ZIP code.
    pub address_zip: String,
    /// Registration status (Valid, Cancelled, Expired, etc.).
    pub status_code: String,
    /// Code describing airworthiness certificate type.
    pub cert_issue_date: String,
    /// Air speed class.
    pub air_worth_date: String,
    /// Model year manufactured.
    pub year_mfr: String,
    /// Registration type code: 1=Individual, 2=Partnership, 3=Corporation, etc.
    pub type_registrant: String,
    /// Fractional ownership flag.
    pub fractional_ownership: bool,
    /// Mode S IAICAO address (hex transponder code).
    pub mode_s_code_hex: String,
}

/// Parse FAA MASTER.txt CSV text into aircraft registration records.
///
/// The file uses fixed 30-column CSV with no header; columns positions are
/// defined by the FAA `ReleasableAircraft` documentation.
///
/// | Col | Field |
/// |-----|-------|
/// | 0 | `N-Number` |
/// | 1 | Serial Number |
/// | 2 | MFR MDL Code |
/// | 3 | Eng Mfr Code |
/// | 4 | Year Mfr |
/// | 5 | Type Registrant |
/// | 6 | Registrant Name |
/// | 7 | Street |
/// | 8 | Street 2 |
/// | 9 | City |
/// | 10 | State |
/// | 11 | Zip Code |
/// | 12 | Region |
/// | 13 | County Code |
/// | 14 | Country Code |
/// | 15 | Last Act Date |
/// | 16 | Cert Issue Date |
/// | 17 | Certification |
/// | 18 | Type Aircraft |
/// | 19 | Type Engine |
/// | 20 | Status Code |
/// | 21 | Mode S Code |
/// | 22 | Fractional Owner |
/// | 23 | Air Worth Date |
/// | 24 | Other Names 1-5 |
/// | 29 | Mode S Hex |
#[must_use]
pub fn parse_master_csv(csv: &str) -> Vec<AircraftRegistration> {
    csv.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(parse_master_row)
        .collect()
}

fn parse_master_row(line: &str) -> Option<AircraftRegistration> {
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 23 {
        return None;
    }

    let get = |i: usize| fields.get(i).map_or("", |s| s.trim()).to_string();

    let n_number = get(0);
    if n_number.is_empty() {
        return None;
    }

    let fractional_ownership = get(22).eq_ignore_ascii_case("Y");

    Some(AircraftRegistration {
        n_number,
        serial_number: get(1),
        mfr_mdl_code: get(2),
        year_mfr: get(4),
        type_registrant: registrant_type_label(&get(5)),
        registrant_name: get(6),
        address_street: get(7),
        address_city: get(9),
        address_state: get(10),
        address_zip: get(11),
        cert_issue_date: get(16),
        status_code: status_code_label(&get(20)),
        mode_s_code_hex: get(29),
        fractional_ownership,
        registrant_type: registrant_type_label(&get(5)),
        air_worth_date: get(23),
    })
}

fn registrant_type_label(code: &str) -> String {
    match code.trim() {
        "1" => "Individual".to_string(),
        "2" => "Partnership".to_string(),
        "3" => "Corporation".to_string(),
        "4" => "Co-Owned".to_string(),
        "5" => "Government".to_string(),
        "7" => "LLC".to_string(),
        "8" => "Non-Citizen Corporation".to_string(),
        "9" => "Non-Citizen Co-Owned".to_string(),
        other => other.to_string(),
    }
}

fn status_code_label(code: &str) -> String {
    match code.trim() {
        "V" => "Valid".to_string(),
        "N" => "Deregistered".to_string(),
        "D" => "Expired Dealer Certificate".to_string(),
        "X" => "Expired".to_string(),
        "Z" => "Pending Cancel".to_string(),
        other => other.to_string(),
    }
}

/// Search CSV text for aircraft registered to a specific name.
///
/// Performs a case-insensitive substring match on the registrant name field.
#[must_use]
pub fn search_by_name<'a>(
    records: &'a [AircraftRegistration],
    query: &str,
) -> Vec<&'a AircraftRegistration> {
    let q = query.to_ascii_lowercase();
    records
        .iter()
        .filter(|r| r.registrant_name.to_ascii_lowercase().contains(&q))
        .collect()
}

/// Fetch and parse the FAA `ReleasableAircraft` bulk CSV.
///
/// **Note**: The bulk file is a ZIP archive (~100 MB). This function fetches
/// and parses MASTER.txt from the archive for full-dataset research. For
/// targeted lookups, prefer parsing a locally cached copy.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails or the response cannot be read.
pub async fn fetch_aircraft_bulk(
    output_dir: &Path,
    filter_name: Option<&str>,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(BULK_ZIP_URL).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let bytes = resp.bytes().await?;
    let csv = extract_master_from_zip(&bytes)?;
    let all_records = parse_master_csv(&csv);

    let filtered: Vec<&AircraftRegistration> = filter_name.map_or_else(
        || all_records.iter().collect(),
        |name| search_by_name(&all_records, name),
    );

    let serialized: Vec<serde_json::Value> = filtered
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let output_path = output_dir.join("faa_aircraft.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "faa_nnumber".into(),
        attribution: None,
    })
}

/// Extract MASTER.txt content from the FAA `ReleasableAircraft` ZIP bytes.
///
/// # Errors
///
/// Returns `Err` if the ZIP cannot be read or MASTER.txt is not found.
fn extract_master_from_zip(bytes: &[u8]) -> Result<String, FetchError> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| FetchError::Parse(format!("ZIP open error: {e}")))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| FetchError::Parse(format!("ZIP entry error: {e}")))?;

        let name = file.name().to_ascii_uppercase();
        let has_txt_ext = std::path::Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"));
        if name.contains("MASTER") && has_txt_ext {
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| FetchError::Parse(format!("ZIP read error: {e}")))?;
            return Ok(content);
        }
    }

    Err(FetchError::Parse(
        "MASTER.txt not found in FAA ZIP archive".to_string(),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // FAA MASTER.txt has no header; 30 comma-delimited fields per row
    const MASTER_FIXTURE: &str = "\
N12345,A1B2C3,9999999,,2015,3,SHELL CORP LLC,123 MAIN ST,,MIAMI,FL,33101,5,025,US,,20150601,1A1,4,0,V,A56789,N,20150601,,,,,,,A56789\n\
N98765,X9Y8Z7,1111111,,1998,1,SMITH JOHN,456 OAK AVE,,DALLAS,TX,75201,4,113,US,,20050312,1A3,1,5,X,B12345,N,20050312,,,,,,,B12345\n\
NBADINP,,,,,,,,,,,,,\n\
";

    #[test]
    fn faa_parses_master_csv_extracts_nnumber_name_status() {
        let records = parse_master_csv(MASTER_FIXTURE);
        // Row 3 is skipped (too few fields)
        assert_eq!(records.len(), 2);

        assert_eq!(records[0].n_number, "N12345");
        assert_eq!(records[0].serial_number, "A1B2C3");
        assert_eq!(records[0].registrant_name, "SHELL CORP LLC");
        assert_eq!(records[0].registrant_type, "Corporation");
        assert_eq!(records[0].status_code, "Valid");
        assert_eq!(records[0].address_state, "FL");
    }

    #[test]
    fn faa_parses_individual_and_deregistered_aircraft() {
        let records = parse_master_csv(MASTER_FIXTURE);

        assert_eq!(records[1].registrant_type, "Individual");
        assert_eq!(records[1].registrant_name, "SMITH JOHN");
        assert_eq!(records[1].status_code, "Expired");
        assert_eq!(records[1].year_mfr, "1998");
        assert_eq!(records[1].address_state, "TX");
    }

    #[test]
    fn faa_extracts_manufacturer_model_serial_and_registration_date() {
        let records = parse_master_csv(MASTER_FIXTURE);

        assert_eq!(records[0].mfr_mdl_code, "9999999");
        assert_eq!(records[0].cert_issue_date, "20150601");
        assert!(!records[0].fractional_ownership);
    }

    #[test]
    fn faa_search_by_name_finds_matching_registrants() {
        let records = parse_master_csv(MASTER_FIXTURE);
        let matches = search_by_name(&records, "SMITH");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].registrant_name, "SMITH JOHN");
    }

    #[test]
    fn faa_handles_deregistered_status_code() {
        let records = parse_master_csv(MASTER_FIXTURE);
        let active = records.iter().filter(|r| r.status_code == "Valid").count();
        let expired = records
            .iter()
            .filter(|r| r.status_code == "Expired")
            .count();

        assert_eq!(active, 1);
        assert_eq!(expired, 1);
    }
}
