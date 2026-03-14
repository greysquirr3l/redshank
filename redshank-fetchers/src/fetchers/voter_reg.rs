//! Voter Registration — State voter file parsers.
//!
//! Free/cheap states with public voter files:
//! - FL: https://dos.fl.gov/elections/data-statistics/
//! - NC: https://www.ncsbe.gov/results-data/voter-registration-data
//! - OH: https://www.ohiosos.gov/elections/voters/
//!
//! Each provides tab-delimited or CSV files (zip download).

use crate::domain::{FetchError, FetchOutput};
use crate::write_ndjson;
use std::path::Path;

/// Parse a tab-delimited voter registration row (NC format).
///
/// NC fields: county_id, county_desc, voter_reg_num, ncid, last_name, first_name,
/// middle_name, name_suffix, status_cd, voter_status_desc, reason_cd, reason_desc,
/// res_street_address, res_city_desc, state_cd, zip_code, mail_addr1, mail_city,
/// mail_state, mail_zipcode, full_phone_number, confidential_ind, registr_dt,
/// race_code, ethnic_code, party_cd, gender_code, birth_year, age_at_year_end,
/// birth_state, drivers_lic, precinct_abbrv, precinct_desc, municipality_abbrv,
/// municipality_desc, ward_abbrv, ward_desc, cong_dist_abbrv, super_court_abbrv,
/// judic_dist_abbrv, nc_senate_abbrv, nc_house_abbrv
pub fn parse_nc_voter_row(row: &str) -> Option<serde_json::Value> {
    let fields: Vec<&str> = row.split('\t').collect();
    if fields.len() < 30 {
        return None;
    }

    Some(serde_json::json!({
        "county": fields[1],
        "voter_reg_num": fields[2],
        "ncid": fields[3],
        "last_name": fields[4],
        "first_name": fields[5],
        "middle_name": fields[6],
        "status": fields[8],
        "street_address": fields[12],
        "city": fields[13],
        "state": fields[14],
        "zip_code": fields[15],
        "registr_dt": fields[21],
        "race_code": fields[22],
        "party_cd": fields[24],
        "gender_code": fields[25],
        "birth_year": fields[26],
    }))
}

/// Parse voter registration rows from a tab-delimited file and write as NDJSON.
pub fn parse_voter_file(
    content: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let records: Vec<serde_json::Value> = content
        .lines()
        .skip(1) // skip header
        .filter_map(parse_nc_voter_row)
        .collect();

    let output_path = output_dir.join("voter_registration.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "voter_reg".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voter_reg_parses_nc_tab_delimited_row() {
        // Simulated NC voter file row with tab-separated fields
        let row = "1\tWAKE\tVR12345\tNC67890\tDOE\tJOHN\tM\t\tA\tACTIVE\t\t\
                    \t123 MAIN ST\tRALEIGH\tNC\t27601\t\t\t\t\t\t2020-01-15\
                    \tW\t\tDEM\tM\t1985\t39\tNC\t\t01-01\tPrecinct 1\t\t\t\t\t\t\t\t\t";
        let result = parse_nc_voter_row(row).unwrap();
        assert_eq!(result["last_name"], "DOE");
        assert_eq!(result["first_name"], "JOHN");
        assert_eq!(result["county"], "WAKE");
        assert_eq!(result["party_cd"], "DEM");
        assert_eq!(result["city"], "RALEIGH");
        assert_eq!(result["birth_year"], "1985");
    }

    #[test]
    fn voter_reg_rejects_short_rows() {
        let row = "too\tfew\tfields";
        assert!(parse_nc_voter_row(row).is_none());
    }
}
