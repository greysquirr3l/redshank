//! Wiki entry and category types.

use serde::{Deserialize, Serialize};

/// Classification for wiki entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WikiCategory {
    /// Campaign finance related.
    CampaignFinance,
    /// Corporate entity.
    Corporate,
    /// Financial data.
    Financial,
    /// Government contracts.
    Contracts,
    /// Lobbying disclosures.
    Lobbying,
    /// Sanctions and enforcement.
    Sanctions,
    /// Court records.
    Courts,
    /// Property records.
    Property,
    /// Individual person / OSINT.
    People,
    /// Other / uncategorised.
    Other(String),
}

/// A single wiki entry (entity page).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiEntry {
    /// Entry title (entity name).
    pub title: String,
    /// Category classification.
    pub category: WikiCategory,
    /// Markdown content body.
    pub content: String,
    /// Cross-references to other entries.
    pub cross_refs: Vec<String>,
}
