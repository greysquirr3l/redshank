//! CMS Open Payments parser and fetcher helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, rate_limit_delay, write_ndjson};
use std::collections::BTreeMap;
use std::path::Path;

const API_BASE: &str = "https://data.cms.gov/resource/nhgx-5qnk.json";
const DEFAULT_LIMIT: u32 = 100;

/// A normalized CMS Open Payments record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OpenPaymentRecord {
    pub physician_name: String,
    pub physician_npi: Option<String>,
    pub specialty: Option<String>,
    pub payer: String,
    pub payment_amount: f64,
    pub payment_date: Option<String>,
    pub nature_of_payment: Option<String>,
    pub payment_category: Option<String>,
    pub associated_product: Option<String>,
    pub year: Option<u32>,
}

/// Aggregated yearly payment totals grouped by payer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PaymentAggregate {
    pub year: u32,
    pub payer: String,
    pub total_amount: f64,
    pub record_count: u32,
}

fn optional_string(record: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| record.get(*key).and_then(serde_json::Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn optional_f64(record: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        record.get(*key).and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        })
    })
}

fn extract_year(record: &serde_json::Value) -> Option<u32> {
    optional_string(record, &["program_year", "payment_year", "year"])
        .and_then(|year| year.parse().ok())
}

fn parse_payment_record(record: &serde_json::Value) -> Option<OpenPaymentRecord> {
    let physician_name = optional_string(
        record,
        &[
            "physician_profile_id",
            "physician_name",
            "covered_recipient_name",
            "recipient_name",
        ],
    )?;
    let payer = optional_string(
        record,
        &[
            "applicable_manufacturer_or_applicable_gpo_making_payment_name",
            "applicable_manufacturer_name",
            "payer_name",
        ],
    )?;
    let payment_amount = optional_f64(
        record,
        &["total_amount_of_payment_usdollars", "payment_amount"],
    )?;

    Some(OpenPaymentRecord {
        physician_name,
        physician_npi: optional_string(record, &["physician_npi", "covered_recipient_npi"]),
        specialty: optional_string(record, &["physician_specialty", "specialty"]),
        payer,
        payment_amount,
        payment_date: optional_string(record, &["date_of_payment", "payment_date"]),
        nature_of_payment: optional_string(
            record,
            &[
                "nature_of_payment_or_transfer_of_value",
                "nature_of_payment",
            ],
        ),
        payment_category: optional_string(
            record,
            &["dispute_status_for_publication", "payment_category"],
        ),
        associated_product: optional_string(
            record,
            &[
                "name_of_associated_covered_drug_or_biological1",
                "name_of_associated_drug_or_biological",
                "associated_product",
            ],
        ),
        year: extract_year(record),
    })
}

/// Parse CMS Open Payments API results.
#[must_use]
pub fn parse_payments(json: &serde_json::Value) -> Vec<OpenPaymentRecord> {
    json.as_array()
        .map(|records| records.iter().filter_map(parse_payment_record).collect())
        .unwrap_or_default()
}

/// Aggregate payment totals by year and payer.
#[must_use]
pub fn aggregate_payments(records: &[OpenPaymentRecord]) -> Vec<PaymentAggregate> {
    let mut grouped: BTreeMap<(u32, String), (f64, u32)> = BTreeMap::new();

    for record in records.iter().filter(|record| record.year.is_some()) {
        let key = (record.year.unwrap_or_default(), record.payer.clone());
        let entry = grouped.entry(key).or_insert((0.0, 0));
        entry.0 += record.payment_amount;
        entry.1 += 1;
    }

    grouped
        .into_iter()
        .map(
            |((year, payer), (total_amount, record_count))| PaymentAggregate {
                year,
                payer,
                total_amount,
                record_count,
            },
        )
        .collect()
}

/// Fetch CMS Open Payments records for a physician NPI.
///
/// # Errors
///
/// Returns `Err` if the request fails or the server returns a non-success status.
pub async fn fetch_by_npi(
    npi: &str,
    output_dir: &Path,
    rate_limit_ms: u64,
    max_pages: u32,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let max = if max_pages == 0 { u32::MAX } else { max_pages };
    let mut page = 0_u32;
    let mut serialized = Vec::new();

    while page < max {
        let offset = page * DEFAULT_LIMIT;
        let resp = client
            .get(API_BASE)
            .query(&[
                ("physician_npi", npi),
                ("$limit", &DEFAULT_LIMIT.to_string()),
                ("$offset", &offset.to_string()),
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
        let records = parse_payments(&json);
        if records.is_empty() {
            break;
        }

        for record in records {
            serialized.push(
                serde_json::to_value(record).map_err(|err| FetchError::Parse(err.to_string()))?,
            );
        }

        if json
            .as_array()
            .is_none_or(|records| records.len() < DEFAULT_LIMIT as usize)
        {
            break;
        }

        page += 1;
        rate_limit_delay(rate_limit_ms).await;
    }

    let output_path = output_dir.join("cms_open_payments.ndjson");
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "cms-open-payments".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn general_payments_fixture() -> serde_json::Value {
        serde_json::json!([
            {
                "physician_name": "Dr. Alice Carter",
                "physician_npi": "1234567890",
                "physician_specialty": "Cardiology",
                "applicable_manufacturer_or_applicable_gpo_making_payment_name": "PharmaCo",
                "total_amount_of_payment_usdollars": "2500.50",
                "date_of_payment": "2024-05-10",
                "nature_of_payment_or_transfer_of_value": "Consulting Fee",
                "name_of_associated_covered_drug_or_biological1": "CardioX",
                "program_year": "2024"
            },
            {
                "physician_name": "Dr. Alice Carter",
                "physician_npi": "1234567890",
                "physician_specialty": "Cardiology",
                "applicable_manufacturer_or_applicable_gpo_making_payment_name": "PharmaCo",
                "total_amount_of_payment_usdollars": "500.00",
                "date_of_payment": "2024-07-01",
                "nature_of_payment_or_transfer_of_value": "Food and Beverage",
                "program_year": "2024"
            }
        ])
    }

    fn research_payments_fixture() -> serde_json::Value {
        serde_json::json!([
            {
                "physician_name": "Dr. Omar Singh",
                "physician_npi": "5555555555",
                "applicable_manufacturer_or_applicable_gpo_making_payment_name": "BioTrials Inc",
                "total_amount_of_payment_usdollars": "120000.00",
                "date_of_payment": "2023-11-15",
                "nature_of_payment_or_transfer_of_value": "Research",
                "name_of_associated_covered_drug_or_biological1": "TrialDrug-7",
                "program_year": "2023"
            }
        ])
    }

    #[test]
    fn cms_open_payments_parses_general_payments_fixture() {
        let records = parse_payments(&general_payments_fixture());

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].physician_name, "Dr. Alice Carter");
        assert_eq!(records[0].physician_npi.as_deref(), Some("1234567890"));
        assert_eq!(records[0].specialty.as_deref(), Some("Cardiology"));
    }

    #[test]
    fn cms_open_payments_parses_research_payments_fixture() {
        let records = parse_payments(&research_payments_fixture());

        assert_eq!(records.len(), 1);
        assert!((records[0].payment_amount - 120_000.0).abs() < f64::EPSILON);
        assert_eq!(records[0].nature_of_payment.as_deref(), Some("Research"));
    }

    #[test]
    fn cms_open_payments_extracts_payer_amount_and_nature() {
        let records = parse_payments(&general_payments_fixture());

        assert_eq!(records[0].payer, "PharmaCo");
        assert!((records[0].payment_amount - 2_500.50).abs() < f64::EPSILON);
        assert_eq!(
            records[0].nature_of_payment.as_deref(),
            Some("Consulting Fee")
        );
    }

    #[test]
    fn cms_open_payments_aggregates_payments_by_year_and_payer() {
        let records = parse_payments(&general_payments_fixture());
        let aggregates = aggregate_payments(&records);

        assert_eq!(aggregates.len(), 1);
        assert_eq!(aggregates[0].year, 2024);
        assert_eq!(aggregates[0].payer, "PharmaCo");
        assert!((aggregates[0].total_amount - 3_000.50).abs() < f64::EPSILON);
        assert_eq!(aggregates[0].record_count, 2);
    }
}
