//! Multi-chain blockchain explorer parsing and fetch helpers.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const ETHERSCAN_BASE: &str = "https://api.etherscan.io/api";
const BLOCKSTREAM_BASE: &str = "https://blockstream.info/api";

/// A token holding associated with an address.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TokenHolding {
    /// Token symbol.
    pub symbol: String,
    /// Contract or mint address.
    pub token_address: Option<String>,
    /// Human-readable balance.
    pub balance: f64,
}

/// A normalized blockchain transaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct BlockchainTransaction {
    /// Transaction hash or id.
    pub txid: String,
    /// Sender address.
    pub from: Option<String>,
    /// Receiver address.
    pub to: Option<String>,
    /// Native-token amount.
    pub amount: Option<f64>,
    /// Unix timestamp if present.
    pub timestamp: Option<u64>,
}

/// A normalized address summary for explorer results.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AddressSnapshot {
    /// Chain name.
    pub chain: String,
    /// Address queried.
    pub address: String,
    /// Native balance.
    pub native_balance: Option<f64>,
    /// Token holdings.
    pub token_holdings: Vec<TokenHolding>,
    /// Recent transactions.
    pub transactions: Vec<BlockchainTransaction>,
    /// First-seen timestamp.
    pub first_seen: Option<u64>,
    /// Last-active timestamp.
    pub last_active: Option<u64>,
}

fn parse_eth_wei_to_eth(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().map(|wei| wei / 1_000_000_000_000_000_000.0)
}

/// Parse an Etherscan balance response.
#[must_use]
pub fn parse_ethereum_balance(address: &str, json: &serde_json::Value) -> Option<AddressSnapshot> {
    let balance = json
        .get("result")
        .and_then(serde_json::Value::as_str)
        .and_then(parse_eth_wei_to_eth)?;

    Some(AddressSnapshot {
        chain: "ethereum".to_string(),
        address: address.to_string(),
        native_balance: Some(balance),
        token_holdings: Vec::new(),
        transactions: Vec::new(),
        first_seen: None,
        last_active: None,
    })
}

/// Parse a Bitcoin address summary from Blockstream.
#[must_use]
pub fn parse_bitcoin_address(address: &str, json: &serde_json::Value) -> Option<AddressSnapshot> {
    let chain_stats = json.get("chain_stats")?;
    let funded = chain_stats.get("funded_txo_sum").and_then(serde_json::Value::as_u64)?;
    let spent = chain_stats.get("spent_txo_sum").and_then(serde_json::Value::as_u64)?;
    let tx_count = chain_stats.get("tx_count").and_then(serde_json::Value::as_u64);
    let native_balance = sats_to_btc(funded.saturating_sub(spent))?;

    Some(AddressSnapshot {
        chain: "bitcoin".to_string(),
        address: address.to_string(),
        native_balance: Some(native_balance),
        token_holdings: Vec::new(),
        transactions: Vec::new(),
        first_seen: None,
        last_active: tx_count,
    })
}

fn sats_to_btc(satoshis: u64) -> Option<f64> {
    satoshis
        .to_string()
        .parse::<f64>()
        .ok()
        .map(|value| value / 100_000_000.0)
}

/// Parse a Bitcoin transaction list response.
#[must_use]
pub fn parse_bitcoin_transactions(json: &serde_json::Value) -> Vec<BlockchainTransaction> {
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let txid = entry.get("txid")?.as_str()?.to_string();
            let inputs = entry.get("vin").and_then(serde_json::Value::as_array);
            let outputs = entry.get("vout").and_then(serde_json::Value::as_array);
            let from = inputs.and_then(|items| {
                items.first().and_then(|item| {
                    item.get("prevout")
                        .and_then(|prev| prev.get("scriptpubkey_address"))
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                })
            });
            let to = outputs.and_then(|items| {
                items.first().and_then(|item| {
                    item.get("scriptpubkey_address")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string)
                })
            });
            let amount = outputs.and_then(|items| {
                items.first()
                    .and_then(|item| item.get("value"))
                    .and_then(serde_json::Value::as_u64)
                    .and_then(sats_to_btc)
            });
            let timestamp = entry
                .get("status")
                .and_then(|status| status.get("block_time"))
                .and_then(serde_json::Value::as_u64);

            Some(BlockchainTransaction {
                txid,
                from,
                to,
                amount,
                timestamp,
            })
        })
        .collect()
}

/// Parse an ERC-20 token list response.
#[must_use]
pub fn parse_token_holdings(json: &serde_json::Value) -> Vec<TokenHolding> {
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let symbol = entry
                .get("tokenSymbol")
                .or_else(|| entry.get("symbol"))
                .and_then(serde_json::Value::as_str)?
                .to_string();
            let decimals = entry
                .get("tokenDecimal")
                .or_else(|| entry.get("decimals"))
                .and_then(serde_json::Value::as_str)
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(18);
            let raw_balance = entry
                .get("balance")
                .or_else(|| entry.get("tokenBalance"))
                .and_then(serde_json::Value::as_str)?;
            let divisor = 10_f64.powi(i32::try_from(decimals).ok()?);
            let balance = raw_balance.parse::<f64>().ok()? / divisor;

            Some(TokenHolding {
                symbol,
                token_address: entry
                    .get("contractAddress")
                    .and_then(serde_json::Value::as_str)
                    .map(ToString::to_string),
                balance,
            })
        })
        .collect()
}

/// Fetch a chain address snapshot.
///
/// # Errors
///
/// Returns `Err` if the explorer request fails or the response cannot be parsed.
pub async fn fetch_address_snapshot(
    chain: &str,
    address: &str,
    api_key: Option<&str>,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let snapshot = match chain {
        "ethereum" => {
            let key = api_key.ok_or_else(|| {
                FetchError::Other("ethereum explorer requires an etherscan_api_key".to_string())
            })?;
            let resp = client
                .get(ETHERSCAN_BASE)
                .query(&[
                    ("module", "account"),
                    ("action", "balance"),
                    ("address", address),
                    ("tag", "latest"),
                    ("apikey", key),
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
            parse_ethereum_balance(address, &json)
                .ok_or_else(|| FetchError::Parse("could not parse Ethereum balance".to_string()))?
        }
        "bitcoin" => {
            let resp = client
                .get(format!("{BLOCKSTREAM_BASE}/address/{address}"))
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
            parse_bitcoin_address(address, &json)
                .ok_or_else(|| FetchError::Parse("could not parse Bitcoin address".to_string()))?
        }
        _ => {
            return Err(FetchError::Other(format!(
                "unsupported chain '{chain}'; use ethereum or bitcoin"
            )));
        }
    };

    let output_path = output_dir.join("blockchain_explorer.ndjson");
    let records = vec![serde_json::to_value(snapshot).map_err(|err| FetchError::Parse(err.to_string()))?];
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "blockchain_explorer".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn blockchain_explorer_parses_ethereum_address_balance_fixture() {
        let json = serde_json::json!({
            "status": "1",
            "message": "OK",
            "result": "1234500000000000000"
        });

        let snapshot = parse_ethereum_balance("0xabc", &json).unwrap();
        assert_eq!(snapshot.address, "0xabc");
        assert_eq!(snapshot.native_balance, Some(1.2345));
        assert_eq!(snapshot.chain, "ethereum");
    }

    #[test]
    fn blockchain_explorer_parses_bitcoin_address_transaction_history() {
        let json = serde_json::json!([
            {
                "txid": "tx-one",
                "status": {"block_time": 1_710_000_000},
                "vin": [{"prevout": {"scriptpubkey_address": "bc1from"}}],
                "vout": [{"scriptpubkey_address": "bc1to", "value": 125_000_000}]
            }
        ]);

        let transactions = parse_bitcoin_transactions(&json);
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].from.as_deref(), Some("bc1from"));
        assert_eq!(transactions[0].to.as_deref(), Some("bc1to"));
        assert_eq!(transactions[0].amount, Some(1.25));
    }

    #[test]
    fn blockchain_explorer_extracts_token_holdings_from_erc20_list() {
        let json = serde_json::json!([
            {
                "tokenSymbol": "USDC",
                "contractAddress": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                "tokenDecimal": "6",
                "balance": "2534500000"
            }
        ]);

        let holdings = parse_token_holdings(&json);
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].symbol, "USDC");
        assert!((holdings[0].balance - 2_534.5).abs() < f64::EPSILON);
    }
}