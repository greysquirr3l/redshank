//! `DeFi` protocol position parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use chrono::Utc;
use redshank_core::domain::observation::{EntityObservation, ObservationDelta};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const COMPOUND_API_BASE: &str = "https://api.compound.finance/api/v2/account";

/// A normalized `DeFi` position.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct DefiPosition {
    /// Protocol name.
    pub protocol: String,
    /// Position type.
    pub position_type: String,
    /// Asset or pair label.
    pub asset: String,
    /// Deposited, provided, or staked amount.
    pub supplied_amount: Option<f64>,
    /// Borrowed amount if applicable.
    pub borrowed_amount: Option<f64>,
    /// USD value locked or supplied.
    pub usd_value: Option<f64>,
    /// Health factor or risk metric.
    pub health_factor: Option<f64>,
}

/// Parse a Uniswap liquidity position fixture.
#[must_use]
pub fn parse_uniswap_positions(json: &serde_json::Value) -> Vec<DefiPosition> {
    json.get("data")
        .and_then(|value| value.get("positions"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|position| {
            let token0 = position
                .get("pool")
                .and_then(|pool| pool.get("token0"))
                .and_then(|token| token.get("symbol"))
                .and_then(serde_json::Value::as_str)?;
            let token1 = position
                .get("pool")
                .and_then(|pool| pool.get("token1"))
                .and_then(|token| token.get("symbol"))
                .and_then(serde_json::Value::as_str)?;
            Some(DefiPosition {
                protocol: "uniswap".to_string(),
                position_type: "liquidity".to_string(),
                asset: format!("{token0}/{token1}"),
                supplied_amount: position
                    .get("liquidity")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok()),
                borrowed_amount: None,
                usd_value: position
                    .get("depositedToken0")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok())
                    .zip(
                        position
                            .get("depositedToken1")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|value| value.parse::<f64>().ok()),
                    )
                    .map(|(left, right)| left + right),
                health_factor: None,
            })
        })
        .collect()
}

/// Parse an Aave lending position fixture.
#[must_use]
pub fn parse_aave_positions(json: &serde_json::Value) -> Vec<DefiPosition> {
    json.get("data")
        .and_then(|value| value.get("positions"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|position| {
            let asset = position
                .get("reserve")
                .and_then(|reserve| reserve.get("symbol"))
                .and_then(serde_json::Value::as_str)?;
            Some(DefiPosition {
                protocol: "aave".to_string(),
                position_type: "lending".to_string(),
                asset: asset.to_string(),
                supplied_amount: position
                    .get("supplied")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok()),
                borrowed_amount: position
                    .get("borrowed")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok()),
                usd_value: position
                    .get("usdValue")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok()),
                health_factor: position
                    .get("healthFactor")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|value| value.parse::<f64>().ok()),
            })
        })
        .collect()
}

fn snapshot_payload_hash(positions: &[DefiPosition]) -> Result<String, FetchError> {
    let payload = serde_json::to_vec(positions)
        .map_err(|err| FetchError::Parse(format!("serialize defi positions hash: {err}")))?;
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&payload);
    Ok(format!("{:08x}", hasher.finalize()))
}

fn classify_delta(previous: Option<&EntityObservation>, payload_hash: &str) -> ObservationDelta {
    match previous {
        None => ObservationDelta::New,
        Some(prev) if prev.payload_hash == payload_hash => ObservationDelta::Unchanged,
        Some(prev) => ObservationDelta::Changed {
            previous_hash: prev.payload_hash.clone(),
        },
    }
}

fn read_latest_observation(
    path: &Path,
    entity_id: &str,
) -> Result<Option<EntityObservation>, FetchError> {
    if !path.exists() {
        return Ok(None);
    }

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut latest: Option<EntityObservation> = None;

    for line_result in reader.lines() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        let observation: EntityObservation = serde_json::from_str(&line).map_err(|err| {
            FetchError::Parse(format!(
                "parse observation line from {}: {err}",
                path.display()
            ))
        })?;
        if observation.entity_id != entity_id || observation.source_id != "defi_protocols" {
            continue;
        }

        let should_replace = latest
            .as_ref()
            .is_none_or(|current| observation.observed_at > current.observed_at);
        if should_replace {
            latest = Some(observation);
        }
    }

    Ok(latest)
}

fn append_observation(path: &Path, observation: &EntityObservation) -> Result<(), FetchError> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, observation)
        .map_err(|err| FetchError::Parse(format!("serialize observation: {err}")))?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

/// Fetch Compound account positions for an address.
///
/// # Errors
///
/// Returns `Err` if the request fails or the response cannot be normalized.
pub async fn fetch_compound_positions(
    address: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get(COMPOUND_API_BASE)
        .query(&[("addresses[]", address)])
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
        .get("accounts")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let output_path = output_dir.join("defi_protocols.ndjson");
    let observation_path = output_dir.join("defi_protocols_observations.ndjson");

    // Emit PoL observation for this address's DeFi positions snapshot.
    let entity_id = format!("defi:compound:{}", address.to_ascii_lowercase());
    let positions_str = serde_json::to_string(&records)
        .map_err(|err| FetchError::Parse(format!("serialize positions: {err}")))?;
    let payload_hash = snapshot_payload_hash(&[]).map(|_| {
        format!("{:08x}", {
            let mut h = crc32fast::Hasher::new();
            h.update(positions_str.as_bytes());
            h.finalize()
        })
    })?;
    let previous = read_latest_observation(&observation_path, &entity_id)?;
    let delta = classify_delta(previous.as_ref(), &payload_hash);
    let observation = EntityObservation::new(
        entity_id,
        "defi_protocols".to_owned(),
        Utc::now(),
        payload_hash,
        delta,
    );
    append_observation(&observation_path, &observation)?;

    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "defi_protocols".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn defi_protocols_parses_uniswap_liquidity_position_fixture() {
        let json = serde_json::json!({
            "data": {
                "positions": [{
                    "liquidity": "125000.5",
                    "depositedToken0": "12.5",
                    "depositedToken1": "24500.0",
                    "pool": {
                        "token0": {"symbol": "WETH"},
                        "token1": {"symbol": "USDC"}
                    }
                }]
            }
        });

        let positions = parse_uniswap_positions(&json);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].asset, "WETH/USDC");
        assert_eq!(positions[0].position_type, "liquidity");
        assert_eq!(positions[0].usd_value, Some(24_512.5));
    }

    #[test]
    fn defi_protocols_parses_aave_lending_position_fixture() {
        let json = serde_json::json!({
            "data": {
                "positions": [{
                    "reserve": {"symbol": "USDC"},
                    "supplied": "15000.0",
                    "borrowed": "3250.25",
                    "usdValue": "15000.0",
                    "healthFactor": "1.84"
                }]
            }
        });

        let positions = parse_aave_positions(&json);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].asset, "USDC");
        assert_eq!(positions[0].borrowed_amount, Some(3250.25));
        assert_eq!(positions[0].health_factor, Some(1.84));
    }
}
