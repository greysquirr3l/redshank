//! State Secretary of State corporate registry pipelines.
//!
//! Uses stygian-graph pipeline with browser + AI extraction nodes for
//! JS-heavy state portals that lack public APIs.
//!
//! Supported states: Delaware, Wyoming, Nevada, Florida.
//! Pipeline configs stored as TOML, loaded via `include_str!`.

use crate::fallback::{FetchExecutionMode, StygianAvailability, select_execution_mode};

/// All state SOS portals are JS-heavy (no public API — browser required).
const STATE_SOS_IS_JS_HEAVY: bool = true;

/// State SOS pipeline configuration TOML for Delaware ICIS.
pub const PIPELINE_DE: &str = include_str!("../../pipelines/state_sos/delaware.toml");

/// State SOS pipeline configuration TOML for Wyoming.
pub const PIPELINE_WY: &str = include_str!("../../pipelines/state_sos/wyoming.toml");

/// State SOS pipeline configuration TOML for Nevada.
pub const PIPELINE_NV: &str = include_str!("../../pipelines/state_sos/nevada.toml");

/// State SOS pipeline configuration TOML for Florida Sunbiz.
pub const PIPELINE_FL: &str = include_str!("../../pipelines/state_sos/florida.toml");

/// Parsed pipeline configuration for a state SOS scrape.
#[derive(Debug, Clone)]
pub struct StateSosPipeline {
    pub state_code: String,
    pub portal_url: String,
    pub search_selector: String,
    pub result_selector: String,
    pub detail_fields: Vec<String>,
}

/// Parse a state SOS pipeline TOML into a structured config.
///
/// # Errors
///
/// Returns `Err` if the TOML is missing required fields (`state_code`, `portal_url`).
pub fn parse_pipeline_config(toml_str: &str) -> Result<StateSosPipeline, String> {
    // Minimal TOML parser for pipeline configs
    let mut state_code = String::new();
    let mut portal_url = String::new();
    let mut search_selector = String::new();
    let mut result_selector = String::new();
    let mut detail_fields = Vec::new();

    for line in toml_str.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "state_code" => value.clone_into(&mut state_code),
                "portal_url" => value.clone_into(&mut portal_url),
                "search_selector" => value.clone_into(&mut search_selector),
                "result_selector" => value.clone_into(&mut result_selector),
                "detail_fields" => {
                    // Parse TOML array: ["field1", "field2"]
                    let inner = value.trim_start_matches('[').trim_end_matches(']');
                    detail_fields = inner
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    if state_code.is_empty() || portal_url.is_empty() {
        return Err("Missing required fields: state_code, portal_url".into());
    }

    Ok(StateSosPipeline {
        state_code,
        portal_url,
        search_selector,
        result_selector,
        detail_fields,
    })
}

/// Select the fetch execution mode for a state SOS portal.
///
/// All state SOS portals are JS-heavy; this delegates to the T47 policy layer.
#[must_use]
pub const fn execution_mode_for_state_sos(
    availability: &StygianAvailability,
) -> FetchExecutionMode {
    select_execution_mode(STATE_SOS_IS_JS_HEAVY, availability)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use crate::fallback::{FetchExecutionMode, StygianAvailability};

    #[test]
    fn state_sos_pipeline_config_loads_and_validates() {
        // Validate all 4 pipeline configs load without error
        for (name, toml) in [
            ("DE", PIPELINE_DE),
            ("WY", PIPELINE_WY),
            ("NV", PIPELINE_NV),
            ("FL", PIPELINE_FL),
        ] {
            let config = parse_pipeline_config(toml)
                .unwrap_or_else(|e| panic!("Failed to parse {name} pipeline: {e}"));
            assert_eq!(config.state_code, name);
            assert!(!config.portal_url.is_empty());
            assert!(!config.detail_fields.is_empty());
        }
    }

    #[test]
    fn state_sos_selects_fallback_mode_when_stygian_available() {
        let availability = StygianAvailability::Available {
            endpoint_url: "http://127.0.0.1:8787/health".into(),
        };
        let mode = execution_mode_for_state_sos(&availability);
        assert_eq!(mode, FetchExecutionMode::StygianMcpFallback);
    }

    #[test]
    fn state_sos_selects_fail_soft_when_stygian_unavailable() {
        let availability = StygianAvailability::Unavailable(
            crate::fallback::StygianUnavailableReason::FeatureDisabled,
        );
        let mode = execution_mode_for_state_sos(&availability);
        assert_eq!(mode, FetchExecutionMode::FailSoft);
    }
}
