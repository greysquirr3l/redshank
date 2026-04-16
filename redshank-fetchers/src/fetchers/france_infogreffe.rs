//! France company registry parsing for Infogreffe and Bodacc.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const PAPPERS_SEARCH_URL: &str = "https://api.pappers.fr/v2/recherche";

/// A normalized French company record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FranceCompanyRecord {
    /// Corporate name.
    pub denomination_sociale: String,
    /// Legal form.
    pub forme_juridique: Option<String>,
    /// SIREN identifier.
    pub siren: Option<String>,
    /// Registered office.
    pub siege_social: Option<String>,
    /// Main executive or gérant.
    pub gerant: Option<String>,
}

/// A Bodacc legal announcement.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct BodaccAnnouncement {
    /// Announcement type.
    pub announcement_type: Option<String>,
    /// Court or publication source.
    pub tribunal: Option<String>,
    /// Free-text summary.
    pub summary: Option<String>,
}

fn extract_between(haystack: &str, start: &str, end: &str) -> Option<String> {
    let from = haystack.find(start)? + start.len();
    let remainder = haystack.get(from..)?;
    let to = remainder.find(end)?;
    Some(remainder[..to].trim().to_string())
}

/// Parse an Infogreffe or Pappers-style company fixture.
#[must_use]
pub fn parse_infogreffe_company(document: &str) -> Option<FranceCompanyRecord> {
    Some(FranceCompanyRecord {
        denomination_sociale: extract_between(document, "data-denomination=\"", "\"")?,
        forme_juridique: extract_between(document, "data-forme-juridique=\"", "\""),
        siren: extract_between(document, "data-siren=\"", "\""),
        siege_social: extract_between(document, "data-siege-social=\"", "\""),
        gerant: extract_between(document, "data-gerant=\"", "\""),
    })
}

/// Parse a Bodacc legal announcement fixture.
#[must_use]
pub fn parse_bodacc_announcement(document: &str) -> Option<BodaccAnnouncement> {
    Some(BodaccAnnouncement {
        announcement_type: extract_between(document, "data-announcement-type=\"", "\""),
        tribunal: extract_between(document, "data-tribunal=\"", "\""),
        summary: extract_between(document, "data-summary=\"", "\""),
    })
}

/// Fetch French company search results.
///
/// # Errors
///
/// Returns `Err` if the request fails.
pub async fn fetch_france_infogreffe(
    company_name: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;
    let resp = client
        .get(PAPPERS_SEARCH_URL)
        .query(&[("q", company_name)])
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
    let output_path = output_dir.join("france_infogreffe.ndjson");
    let count = write_ndjson(&output_path, &[json])?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "france_infogreffe".into(),
        attribution: None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn france_fixture() -> &'static str {
        r#"
        <main data-denomination="Acme France SARL" data-forme-juridique="SARL" data-siren="750100001" data-siege-social="12 rue du Commerce, Paris" data-gerant="Claire Martin"></main>
        "#
    }

    fn bodacc_fixture() -> &'static str {
        r#"
        <article data-announcement-type="Liquidation judiciaire" data-tribunal="Tribunal de commerce de Paris" data-summary="Ouverture d'une procedure de liquidation judiciaire pour Acme France SARL."></article>
        "#
    }

    #[test]
    fn france_infogreffe_fetcher_parses_sarl_company_fixture() {
        let company = parse_infogreffe_company(france_fixture()).unwrap();
        assert_eq!(company.denomination_sociale, "Acme France SARL");
        assert_eq!(company.forme_juridique.as_deref(), Some("SARL"));
        assert_eq!(company.siren.as_deref(), Some("750100001"));
    }

    #[test]
    fn france_infogreffe_fetcher_extracts_gerant_and_siege_social() {
        let company = parse_infogreffe_company(france_fixture()).unwrap();
        assert_eq!(company.gerant.as_deref(), Some("Claire Martin"));
        assert!(company.siege_social.as_deref().unwrap().contains("Paris"));
    }

    #[test]
    fn france_bodacc_fetcher_parses_legal_announcement_fixture() {
        let announcement = parse_bodacc_announcement(bodacc_fixture()).unwrap();
        assert_eq!(
            announcement.announcement_type.as_deref(),
            Some("Liquidation judiciaire")
        );
        assert!(
            announcement
                .summary
                .as_deref()
                .unwrap()
                .contains("liquidation judiciaire")
        );
    }
}
