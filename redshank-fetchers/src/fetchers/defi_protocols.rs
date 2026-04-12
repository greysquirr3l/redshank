//! DeFi protocol position parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const COMPOUND_API_BASE: &str = "https://api.compound.finance/api/v2/account";

/// A normalized DeFi position.
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