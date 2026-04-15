//! Exchange proof-of-reserves and compliance report parsing.

use crate::domain::{FetchError, FetchOutput};
use crate::fetchers::pol_sidecar;
use crate::{build_client, write_ndjson};
use chrono::Utc;
use redshank_core::domain::observation::EntityObservation;
use std::path::Path;

// Re-export pol_sidecar helpers so other fetchers can import from a canonical location.
// The helpers should be moved here in future refactoring to avoid duplication.
pub use crate::fetchers::pol_sidecar::{
    append_observation, classify_delta, read_latest_observation, snapshot_payload_hash,
};

/// A normalized exchange transparency report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ExchangeTransparencyReport {
    /// Exchange name.
    pub exchange: String,
    /// Reserve assets reported.
    pub reserve_assets: Vec<(String, f64)>,
    /// Claimed customer liabilities.
    pub customer_liabilities: Option<f64>,
    /// Ratio of reserves to liabilities.
    pub reserve_ratio: Option<f64>,
    /// Last attestation date.
    pub attestation_date: Option<String>,
    /// AML compliance statement or registration summary.
    pub aml_summary: Option<String>,
    /// `FinCEN` MSB registration or similar identifier.
    pub registration_id: Option<String>,
}

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let from = haystack.find(start)? + start.len();
    let remainder = haystack.get(from..)?;
    let to = remainder.find(end)?;
    Some(remainder[..to].trim().to_string())
}

fn collect_attr_values(html: &str, attr: &str) -> Vec<String> {
    let marker = format!("{attr}=\"");
    let mut values = Vec::new();
    let mut remainder = html;

    while let Some(idx) = remainder.find(&marker) {
        let after = &remainder[idx + marker.len()..];
        let Some(end_idx) = after.find('"') else {
            break;
        };
        let value = after[..end_idx].trim();
        if !value.is_empty() {
            values.push(value.to_string());
        }
        remainder = &after[end_idx + 1..];
    }

    values
}

/// Parse an exchange proof-of-reserves page fixture.
#[must_use]
pub fn parse_proof_of_reserves(html: &str) -> Option<ExchangeTransparencyReport> {
    let exchange = extract_between(html, "data-exchange=\"", "\"")?;
    let asset_symbols = collect_attr_values(html, "data-reserve-asset");
    let asset_amounts = collect_attr_values(html, "data-reserve-amount");
    let reserve_assets = asset_symbols
        .iter()
        .enumerate()
        .filter_map(|(index, symbol)| {
            asset_amounts
                .get(index)
                .and_then(|amount| amount.parse::<f64>().ok())
                .map(|amount| (symbol.clone(), amount))
        })
        .collect::<Vec<_>>();
    let customer_liabilities = extract_between(html, "data-customer-liabilities=\"", "\"")
        .and_then(|value| value.parse::<f64>().ok());
    let reserve_ratio = extract_between(html, "data-reserve-ratio=\"", "\"")
        .and_then(|value| value.parse::<f64>().ok());

    Some(ExchangeTransparencyReport {
        exchange,
        reserve_assets,
        customer_liabilities,
        reserve_ratio,
        attestation_date: extract_between(html, "data-attestation-date=\"", "\""),
        aml_summary: None,
        registration_id: None,
    })
}

/// Parse an AML compliance report fixture.
#[must_use]
pub fn parse_aml_report(json: &serde_json::Value) -> Option<ExchangeTransparencyReport> {
    let exchange = json.get("exchange")?.as_str()?.to_string();
    let registration_id = json
        .get("registration")
        .and_then(|value| value.get("msb_number"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);

    Some(ExchangeTransparencyReport {
        exchange,
        reserve_assets: Vec::new(),
        customer_liabilities: None,
        reserve_ratio: None,
        attestation_date: json
            .get("attestation_date")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        aml_summary: json
            .get("aml_summary")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        registration_id,
    })
}

/// Fetch and persist an exchange transparency page.
///
/// # Errors
///
/// Returns `Err` if the page request fails or the report cannot be parsed.
pub async fn fetch_exchange_transparency(
    url: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client.get(url).send().await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    let body = resp.text().await?;
    let report = parse_proof_of_reserves(&body).ok_or_else(|| {
        FetchError::Parse("could not parse exchange transparency page".to_string())
    })?;
    let output_path = output_dir.join("exchange_transparency.ndjson");
    let observation_path = output_dir.join("exchange_transparency_observations.ndjson");

    // Emit PoL observation for this exchange's transparency report.
    let entity_id = format!("exchange:{}", report.exchange.to_ascii_lowercase());
    let payload_hash = pol_sidecar::snapshot_payload_hash(&report)?;
    let previous = pol_sidecar::read_latest_observation(
        &observation_path,
        &entity_id,
        "exchange_transparency",
    )?;
    let delta = pol_sidecar::classify_delta(previous.as_ref(), &payload_hash);
    let observation = EntityObservation::new(
        entity_id,
        "exchange_transparency".to_owned(),
        Utc::now(),
        payload_hash,
        delta,
    );
    pol_sidecar::append_observation(&observation_path, &observation)?;

    let records =
        vec![serde_json::to_value(report).map_err(|err| FetchError::Parse(err.to_string()))?];
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "exchange_transparency".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn exchange_transparency_parses_proof_of_reserves_attestation() {
        let html = r#"
        <main data-exchange="Kraken" data-customer-liabilities="154000.0" data-reserve-ratio="1.08" data-attestation-date="2026-01-15"></main>
        <div data-reserve-asset="BTC" data-reserve-amount="8450.5"></div>
        <div data-reserve-asset="ETH" data-reserve-amount="62875.0"></div>
        "#;

        let report = parse_proof_of_reserves(html).unwrap();
        assert_eq!(report.exchange, "Kraken");
        assert_eq!(report.reserve_assets.len(), 2);
        assert_eq!(report.reserve_ratio, Some(1.08));
    }

    #[test]
    fn exchange_transparency_parses_aml_compliance_report_fixture() {
        let json = serde_json::json!({
            "exchange": "Kraken",
            "attestation_date": "2026-01-15",
            "aml_summary": "FinCEN-registered MSB with quarterly sanctions screening review.",
            "registration": {
                "msb_number": "31000234567890"
            }
        });

        let report = parse_aml_report(&json).unwrap();
        assert_eq!(report.exchange, "Kraken");
        assert_eq!(report.registration_id.as_deref(), Some("31000234567890"));
        assert!(
            report
                .aml_summary
                .as_deref()
                .unwrap()
                .contains("sanctions screening")
        );
    }
}
