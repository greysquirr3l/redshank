//! RDAP — Registration Data Access Protocol (WHOIS successor).
//!
//! Domain: `https://rdap.org/domain/{domain}` or Verisign for `.com`
//! IP: `https://rdap.arin.net/registry/ip/{ip}`
//! No auth required.

use crate::domain::{FetchError, FetchOutput};
use crate::{build_client, write_ndjson};
use std::path::Path;

const RDAP_DOMAIN_URL: &str = "https://rdap.org/domain";
const RDAP_IP_URL: &str = "https://rdap.arin.net/registry/ip";

/// Fetch RDAP data for a domain name.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_domain_rdap(domain: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    let resp = client
        .get(format!("{RDAP_DOMAIN_URL}/{domain}"))
        .header("Accept", "application/rdap+json")
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let record = extract_domain_fields(&json);

    let output_path = output_dir.join("rdap_domain.ndjson");
    let count = write_ndjson(&output_path, &[record])?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "rdap".into(),
        attribution: None,
    })
}

/// Fetch RDAP data for an IP address.
///
/// # Errors
///
/// Returns `Err` if the HTTP request fails, the server returns a non-success
/// status, or the response cannot be parsed.
pub async fn fetch_ip_rdap(ip: &str, output_dir: &Path) -> Result<FetchOutput, FetchError> {
    let client = build_client()?;

    let resp = client
        .get(format!("{RDAP_IP_URL}/{ip}"))
        .header("Accept", "application/rdap+json")
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(FetchError::ApiError {
            status: status.as_u16(),
            body: text,
        });
    }

    let json: serde_json::Value = resp.json().await?;
    let records = vec![json];

    let output_path = output_dir.join("rdap_ip.ndjson");
    let count = write_ndjson(&output_path, &records)?;

    Ok(FetchOutput {
        records_written: count,
        output_path,
        source_name: "rdap".into(),
        attribution: None,
    })
}

/// Extract key fields from an RDAP domain response.
#[must_use]
pub fn extract_domain_fields(json: &serde_json::Value) -> serde_json::Value {
    let registrar = json
        .get("entities")
        .and_then(|e| e.as_array())
        .and_then(|entities| {
            entities.iter().find(|e| {
                e.get("roles")
                    .and_then(|r| r.as_array())
                    .is_some_and(|roles| roles.iter().any(|r| r.as_str() == Some("registrar")))
            })
        })
        .and_then(|e| {
            e.get("vcardArray")
                .and_then(|v| v.get(1))
                .and_then(|cards| cards.as_array())
                .and_then(|cards| {
                    cards.iter().find_map(|card| {
                        let arr = card.as_array()?;
                        if arr.first()?.as_str() == Some("fn") {
                            arr.get(3)?.as_str().map(String::from)
                        } else {
                            None
                        }
                    })
                })
        })
        .unwrap_or_default();

    let creation_date = json
        .get("events")
        .and_then(|e| e.as_array())
        .and_then(|events| {
            events.iter().find_map(|ev| {
                if ev.get("eventAction")?.as_str() == Some("registration") {
                    ev.get("eventDate")?.as_str().map(String::from)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();

    let expiration_date = json
        .get("events")
        .and_then(|e| e.as_array())
        .and_then(|events| {
            events.iter().find_map(|ev| {
                if ev.get("eventAction")?.as_str() == Some("expiration") {
                    ev.get("eventDate")?.as_str().map(String::from)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();

    let nameservers: Vec<String> = json
        .get("nameservers")
        .and_then(|ns| ns.as_array())
        .map(|ns| {
            ns.iter()
                .filter_map(|n| n.get("ldhName").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    serde_json::json!({
        "ldhName": json.get("ldhName").and_then(|v| v.as_str()).unwrap_or(""),
        "registrar": registrar,
        "creation_date": creation_date,
        "expiration_date": expiration_date,
        "nameservers": nameservers,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn rdap_parses_domain_fixture_and_extracts_registrar() {
        let fixture = serde_json::json!({
            "ldhName": "example.com",
            "entities": [
                {
                    "roles": ["registrar"],
                    "vcardArray": ["vcard", [
                        ["version", {}, "text", "4.0"],
                        ["fn", {}, "text", "Example Registrar Inc."]
                    ]]
                }
            ],
            "events": [
                {"eventAction": "registration", "eventDate": "1995-08-14T00:00:00Z"},
                {"eventAction": "expiration", "eventDate": "2025-08-13T00:00:00Z"}
            ],
            "nameservers": [
                {"ldhName": "ns1.example.com"},
                {"ldhName": "ns2.example.com"}
            ]
        });
        let result = extract_domain_fields(&fixture);
        assert_eq!(result["ldhName"], "example.com");
        assert_eq!(result["registrar"], "Example Registrar Inc.");
        assert_eq!(result["creation_date"], "1995-08-14T00:00:00Z");
        let ns = result["nameservers"].as_array().unwrap();
        assert_eq!(ns.len(), 2);
        assert_eq!(ns[0], "ns1.example.com");
    }
}
