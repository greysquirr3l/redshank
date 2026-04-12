//! Tornado Cash screening and risk scoring.

use crate::fetchers::blockchain_explorer::BlockchainTransaction;

const SANCTIONED_TORNADO_ADDRESSES: &[&str] = &[
    "0xd90e2f925da726b50c4ed8d0fb90ad053324f31b",
    "0x47ce0c6ed4d8fef0d7a846b1f6f0d3f0f7c2c6d0",
    "0x910cbd523d972eb0a6f4cae4618ad62622b39dbf",
];

/// Tornado interaction screening result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct TornadoScreeningResult {
    /// Screened address.
    pub address: String,
    /// Overall risk score from 0.0 to 1.0.
    pub risk_score: f64,
    /// Whether direct interaction with a sanctioned Tornado address was found.
    pub direct_tornado_interaction: bool,
    /// Approximate hops from Tornado.
    pub hops_from_tornado: Option<u8>,
    /// Sanctioned addresses touched in the observed graph.
    pub sanctioned_addresses_touched: Vec<String>,
}

fn normalize_address(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Check whether an address is a known Tornado Cash contract.
#[must_use]
pub fn is_known_tornado_address(address: &str) -> bool {
    let normalized = normalize_address(address);
    SANCTIONED_TORNADO_ADDRESSES.contains(&normalized.as_str())
}

/// Screen an address against a set of observed transactions.
#[must_use]
pub fn screen_transactions(
    address: &str,
    transactions: &[BlockchainTransaction],
) -> TornadoScreeningResult {
    let normalized_address = normalize_address(address);
    let mut direct = false;
    let mut sanctioned = Vec::new();
    let mut one_hop = false;

    for transaction in transactions {
        let counterparties = [transaction.from.as_deref(), transaction.to.as_deref()];
        for counterparty in counterparties.into_iter().flatten() {
            let normalized = normalize_address(counterparty);
            if is_known_tornado_address(&normalized) {
                if normalized == normalized_address {
                    direct = true;
                } else {
                    one_hop = true;
                }
                if !sanctioned.contains(&normalized) {
                    sanctioned.push(normalized);
                }
            }
        }
    }

    let (risk_score, hops_from_tornado) = if direct {
        (1.0, Some(0))
    } else if one_hop {
        (0.72, Some(1))
    } else {
        (0.0, None)
    };

    TornadoScreeningResult {
        address: address.to_string(),
        risk_score,
        direct_tornado_interaction: direct,
        hops_from_tornado,
        sanctioned_addresses_touched: sanctioned,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn transaction_fixture() -> Vec<BlockchainTransaction> {
        vec![
            BlockchainTransaction {
                txid: "tx-direct".to_string(),
                from: Some("0xd90e2f925da726b50c4ed8d0fb90ad053324f31b".to_string()),
                to: Some("0xinvestigator".to_string()),
                amount: Some(2.0),
                timestamp: Some(1_700_000_000),
            },
            BlockchainTransaction {
                txid: "tx-hop".to_string(),
                from: Some("0xintermediary".to_string()),
                to: Some("0x47ce0c6ed4d8fef0d7a846b1f6f0d3f0f7c2c6d0".to_string()),
                amount: Some(0.5),
                timestamp: Some(1_700_100_000),
            },
        ]
    }

    #[test]
    fn tornado_screening_checks_address_against_known_deposit_addresses() {
        assert!(is_known_tornado_address(
            "0xd90e2f925DA726b50C4Ed8D0Fb90Ad053324F31b"
        ));
        assert!(!is_known_tornado_address("0x1234567890abcdef"));
    }

    #[test]
    fn tornado_screening_returns_confidence_score_for_mixer_interaction() {
        let result = screen_transactions(
            "0xd90e2f925DA726b50C4Ed8D0Fb90Ad053324F31b",
            &transaction_fixture(),
        );

        assert!(result.direct_tornado_interaction);
        assert_eq!(result.hops_from_tornado, Some(0));
        assert_eq!(result.risk_score, 1.0);
        assert_eq!(result.sanctioned_addresses_touched.len(), 2);
    }
}
