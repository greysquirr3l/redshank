//! Property valuation data — estimated values, sale history, and ownership.
//!
//! Supports parsing structured JSON responses from valuation APIs and
//! semi-structured data extracted via stygian-browser AI extraction.
//! For Zillow/Redfin scraping, use stygian-browser with anti-bot stealth.

use crate::domain::{FetchError, FetchOutput};
use crate::write_ndjson;
use std::path::Path;

/// A property valuation record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyValuation {
    /// Full property address.
    pub address: String,
    /// City.
    pub city: Option<String>,
    /// State abbreviation.
    pub state: Option<String>,
    /// ZIP code.
    pub zip: Option<String>,
    /// Algorithmic valuation estimate (Zestimate, Redfin Estimate, etc.) in USD.
    pub estimated_value: Option<i64>,
    /// Source of the estimate ("Zillow", "Redfin", "Assessor", etc.).
    pub estimate_source: Option<String>,
    /// Last sale price in USD.
    pub last_sale_price: Option<i64>,
    /// Last sale date (ISO 8601).
    pub last_sale_date: Option<String>,
    /// Property type ("Single Family", "Condo", "Multi-family", etc.).
    pub property_type: Option<String>,
    /// Bedrooms.
    pub bedrooms: Option<u32>,
    /// Bathrooms.
    pub bathrooms: Option<f32>,
    /// Square footage.
    pub sqft: Option<u32>,
    /// Year built.
    pub year_built: Option<u32>,
    /// Listed owner name (if available).
    pub owner: Option<String>,
    /// Price history entries (most recent first).
    pub price_history: Vec<PriceHistoryEntry>,
}

/// A single entry in a property's price history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriceHistoryEntry {
    /// Date of the event (ISO 8601).
    pub date: String,
    /// Price in USD at the time of the event.
    pub price: Option<i64>,
    /// Event type ("Sold", "Listed", "Delisted", "Price Change", "Pending").
    pub event: String,
    /// Source ("MLS", "Public Record", "Zillow", etc.).
    pub source: Option<String>,
}

/// Parse property valuation data from a Zillow-style JSON structure.
///
/// Handles both the legacy Zillow API format and the GraphQL response shape.
#[must_use]
pub fn parse_zillow_json(json: &serde_json::Value) -> Option<PropertyValuation> {
    // Navigate common Zillow response paths
    let props = json
        .get("props")
        .or_else(|| json.get("property"))
        .or_else(|| json.get("data").and_then(|d| d.get("property")))
        .unwrap_or(json);

    let str_field = |key: &str, alt: &str| -> Option<String> {
        props
            .get(key)
            .or_else(|| props.get(alt))
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };

    let i64_field = |key: &str, alt: &str| -> Option<i64> {
        props
            .get(key)
            .or_else(|| props.get(alt))
            .and_then(serde_json::Value::as_i64)
    };

    let u32_field = |key: &str, alt: &str| -> Option<u32> {
        props
            .get(key)
            .or_else(|| props.get(alt))
            .and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };

    let address = str_field("address", "streetAddress")?;

    let price_history = props
        .get("priceHistory")
        .or_else(|| props.get("price_history"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let date = e
                        .get("date")
                        .and_then(serde_json::Value::as_str)
                        .map(String::from)?;
                    let price = e.get("price").and_then(serde_json::Value::as_i64);
                    let event = e
                        .get("event")
                        .or_else(|| e.get("eventType"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Unknown")
                        .to_string();
                    let source = e
                        .get("source")
                        .and_then(serde_json::Value::as_str)
                        .map(String::from);
                    Some(PriceHistoryEntry {
                        date,
                        price,
                        event,
                        source,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let bathrooms = props
        .get("bathrooms")
        .or_else(|| props.get("baths"))
        .and_then(serde_json::Value::as_f64)
        .map(|v| {
            #[allow(clippy::cast_possible_truncation)]
            let result = v as f32;
            result
        });

    Some(PropertyValuation {
        address,
        city: str_field("city", "addressCity"),
        state: str_field("state", "addressState"),
        zip: str_field("zipcode", "zip"),
        estimated_value: i64_field("zestimate", "estimatedValue"),
        estimate_source: Some("Zillow".to_string()),
        last_sale_price: i64_field("lastSalePrice", "last_sale_price"),
        last_sale_date: str_field("lastSaleDate", "last_sale_date"),
        property_type: str_field("propertyType", "homeType"),
        bedrooms: u32_field("bedrooms", "beds"),
        bathrooms,
        sqft: u32_field("livingArea", "sqft"),
        year_built: u32_field("yearBuilt", "year_built"),
        owner: str_field("owner", "ownerName"),
        price_history,
    })
}

/// Write property valuation records to NDJSON.
///
/// # Errors
///
/// Returns `Err` if serialization or I/O fails.
pub fn write_valuation_records(
    records: &[PropertyValuation],
    output_dir: &Path,
    filename: &str,
) -> Result<FetchOutput, FetchError> {
    let serialized: Vec<serde_json::Value> = records
        .iter()
        .filter_map(|r| serde_json::to_value(r).ok())
        .collect();

    let output_path = output_dir.join(filename);
    let count = write_ndjson(&output_path, &serialized)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "property_valuation".into(),
        attribution: None,
    })
}

/// Configuration for a stygian-browser property valuation pipeline.
///
/// Used when `stygian-browser` feature is enabled to drive headless browser
/// scraping of Zillow, Redfin, or county assessor portals.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyScrapeConfig {
    /// Property address to look up.
    pub address: String,
    /// Source to scrape ("zillow", "redfin", "assessor").
    pub source: String,
    /// Rate limit in milliseconds between requests (minimum 10,000 for Zillow/Redfin).
    pub rate_limit_ms: u64,
    /// Output directory for NDJSON results.
    pub output_dir: String,
}

impl PropertyScrapeConfig {
    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the address is empty, source is unsupported,
    /// or rate limit is below minimum for Zillow/Redfin.
    pub fn validate(&self) -> Result<(), FetchError> {
        if self.address.trim().is_empty() {
            return Err(FetchError::Other("address must not be empty".to_string()));
        }
        let source = self.source.to_ascii_lowercase();
        let supported = ["zillow", "redfin", "assessor"];
        if !supported.contains(&source.as_str()) {
            return Err(FetchError::Other(format!(
                "unsupported source '{}'; use one of: {supported:?}",
                self.source
            )));
        }
        // Zillow/Redfin require ≥ 10s rate limit to avoid blocks
        if (source == "zillow" || source == "redfin") && self.rate_limit_ms < 10_000 {
            return Err(FetchError::Other(format!(
                "rate_limit_ms must be >= 10000 for {} to avoid anti-bot detection",
                self.source
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn zillow_fixture() -> serde_json::Value {
        serde_json::json!({
            "address": "123 Main Street",
            "city": "Miami",
            "state": "FL",
            "zipcode": "33101",
            "zestimate": 4_500_000,
            "lastSalePrice": 3_200_000,
            "lastSaleDate": "2021-08-15",
            "propertyType": "Single Family",
            "bedrooms": 5,
            "bathrooms": 4.5,
            "livingArea": 5200,
            "yearBuilt": 2005,
            "owner": "LUXURY HOLDINGS LLC",
            "priceHistory": [
                {
                    "date": "2021-08-15",
                    "price": 3_200_000,
                    "event": "Sold",
                    "source": "Public Record"
                },
                {
                    "date": "2021-07-01",
                    "price": 3_350_000,
                    "event": "Listed",
                    "source": "MLS"
                }
            ]
        })
    }

    #[test]
    fn property_valuation_config_loads_without_launching_browser() {
        let cfg = PropertyScrapeConfig {
            address: "123 Main St, Miami FL 33101".to_string(),
            source: "zillow".to_string(),
            rate_limit_ms: 15_000,
            output_dir: "/tmp".to_string(),
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn property_valuation_config_rejects_low_rate_limit_for_zillow() {
        let cfg = PropertyScrapeConfig {
            address: "123 Main St".to_string(),
            source: "zillow".to_string(),
            rate_limit_ms: 1_000,
            output_dir: "/tmp".to_string(),
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn property_valuation_config_rejects_unsupported_source() {
        let cfg = PropertyScrapeConfig {
            address: "123 Main St".to_string(),
            source: "trulia".to_string(),
            rate_limit_ms: 15_000,
            output_dir: "/tmp".to_string(),
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn property_valuation_extracts_estimated_value_last_sale_price_ownership() {
        let json = zillow_fixture();
        let prop = parse_zillow_json(&json).unwrap();

        assert_eq!(prop.address, "123 Main Street");
        assert_eq!(prop.estimated_value, Some(4_500_000));
        assert_eq!(prop.last_sale_price, Some(3_200_000));
        assert_eq!(prop.owner.as_deref(), Some("LUXURY HOLDINGS LLC"));
    }

    #[test]
    fn property_valuation_extracts_property_details_and_price_history() {
        let json = zillow_fixture();
        let prop = parse_zillow_json(&json).unwrap();

        assert_eq!(prop.bedrooms, Some(5));
        assert!((prop.bathrooms.unwrap() - 4.5_f32).abs() < f32::EPSILON);
        assert_eq!(prop.sqft, Some(5200));
        assert_eq!(prop.year_built, Some(2005));
        assert_eq!(prop.price_history.len(), 2);
        assert_eq!(prop.price_history[0].event, "Sold");
    }
}
