//! Reverse phone lookup (basic/free mode).
//!
//! This fetcher intentionally avoids paid identity APIs.
//! It performs normalization and lightweight metadata inference only.

use crate::domain::{FetchError, FetchOutput};
use crate::write_ndjson;
use std::path::Path;

/// Normalize a phone string into E.164-like format when possible.
#[must_use]
pub fn normalize_phone_number(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut normalized = String::new();
    for c in trimmed.chars() {
        if c.is_ascii_digit() || (c == '+' && normalized.is_empty()) {
            normalized.push(c);
        }
    }

    if let Some(rest) = normalized.strip_prefix("00") {
        normalized = format!("+{rest}");
    }

    if normalized.starts_with('+') {
        let digits = normalized.trim_start_matches('+');
        if (8..=15).contains(&digits.len()) {
            return Some(normalized);
        }
        return None;
    }

    if normalized.len() == 10 {
        return Some(format!("+1{normalized}"));
    }

    if (8..=15).contains(&normalized.len()) {
        return Some(format!("+{normalized}"));
    }

    None
}

/// Best-effort country hint from E.164 prefix.
#[must_use]
pub fn infer_country_hint(e164: &str) -> Option<&'static str> {
    let digits = e164.strip_prefix('+')?;

    if digits.starts_with('1') {
        return Some("North America (+1)");
    }
    if digits.starts_with("44") {
        return Some("United Kingdom (+44)");
    }
    if digits.starts_with("61") {
        return Some("Australia (+61)");
    }
    if digits.starts_with("49") {
        return Some("Germany (+49)");
    }
    if digits.starts_with("33") {
        return Some("France (+33)");
    }

    Some("Unknown country code")
}

/// Perform a free/basic reverse-phone lookup.
///
/// # Errors
///
/// Returns `Err` when the phone value cannot be normalized or output cannot be written.
pub async fn fetch_reverse_phone_basic(
    phone: &str,
    output_dir: &Path,
) -> Result<FetchOutput, FetchError> {
    let normalized = normalize_phone_number(phone)
        .ok_or_else(|| FetchError::Parse("invalid phone number format".to_string()))?;

    let record = serde_json::json!({
        "query": phone,
        "normalized_e164": normalized,
        "country_hint": infer_country_hint(&normalized),
        "result_type": "metadata_only",
        "notes": [
            "No paid identity API used",
            "This result does not assert subscriber identity"
        ]
    });

    let output_path = output_dir.join("reverse_phone_basic.ndjson");
    let records_written = write_ndjson(&output_path, &[record])?;

    Ok(FetchOutput {
        records_written,
        output_path,
        source_name: "reverse_phone_basic".to_string(),
        attribution: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_us_local_number() {
        assert_eq!(
            normalize_phone_number("(415) 555-2671"),
            Some("+14155552671".to_string())
        );
    }

    #[test]
    fn normalizes_international_prefix() {
        assert_eq!(
            normalize_phone_number("0044 20 7946 0958"),
            Some("+442079460958".to_string())
        );
    }

    #[test]
    fn rejects_short_input() {
        assert_eq!(normalize_phone_number("123"), None);
    }
}
