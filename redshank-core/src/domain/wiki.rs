//! Wiki entry and category types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Classification for wiki entries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WikiCategory {
    /// Campaign finance related.
    CampaignFinance,
    /// Government contracts.
    Contracts,
    /// Corporate entity.
    Corporate,
    /// Financial data.
    Financial,
    /// Infrastructure related.
    Infrastructure,
    /// International entities.
    International,
    /// Lobbying disclosures.
    Lobbying,
    /// Nonprofit organisations.
    Nonprofits,
    /// Individual person / OSINT.
    People,
    /// Other / uncategorised.
    Other,
}

/// A single wiki entry (entity page).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiEntry {
    /// File path relative to wiki root.
    pub path: PathBuf,
    /// Entry title (entity name).
    pub title: String,
    /// Category classification.
    pub category: WikiCategory,
    /// Cross-references to other entries.
    pub cross_refs: Vec<String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn wiki_entry_roundtrip_serde() {
        let entry = WikiEntry {
            path: PathBuf::from("corporate/acme-corp.md"),
            title: "Acme Corp".to_string(),
            category: WikiCategory::Corporate,
            cross_refs: vec!["John Doe".to_string()],
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: WikiEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.title, "Acme Corp");
        assert_eq!(restored.category, WikiCategory::Corporate);
    }

    #[test]
    fn all_categories_roundtrip() {
        let cats = vec![
            WikiCategory::CampaignFinance,
            WikiCategory::Contracts,
            WikiCategory::Corporate,
            WikiCategory::Financial,
            WikiCategory::Infrastructure,
            WikiCategory::International,
            WikiCategory::Lobbying,
            WikiCategory::Nonprofits,
            WikiCategory::People,
            WikiCategory::Other,
        ];
        for cat in cats {
            let json = serde_json::to_string(&cat).unwrap();
            let restored: WikiCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, cat);
        }
    }
}
