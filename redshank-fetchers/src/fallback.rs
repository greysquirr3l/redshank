//! Stygian MCP capability detection and fetch execution policy.

use crate::domain::FetchError;
use std::time::Duration;

/// Runtime probe configuration for stygian-mcp availability checks.
#[derive(Debug, Clone)]
pub struct StygianProbeConfig {
    /// Health endpoint URL for stygian-mcp.
    pub endpoint_url: String,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Number of additional attempts after the first, applied to both transport
    /// errors and unhealthy responses.
    pub retries: u8,
}

impl Default for StygianProbeConfig {
    fn default() -> Self {
        Self {
            endpoint_url: "http://127.0.0.1:8787/health".to_string(),
            timeout_ms: 1500,
            retries: 1,
        }
    }
}

/// Why stygian fallback is unavailable at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StygianUnavailableReason {
    /// Binary was built without the `stygian` feature.
    FeatureDisabled,
    /// Health endpoint could not be reached.
    EndpointUnreachable {
        endpoint_url: String,
        message: String,
    },
    /// Endpoint responded but did not report healthy state.
    EndpointUnhealthy {
        endpoint_url: String,
        status: u16,
        body: String,
    },
}

/// Availability state for stygian MCP fallback.
///
/// This enum only has two states: `Available` and `Unavailable`. The
/// "probe not yet run" concept is represented separately by
/// [`redshank_tui::domain::FetcherHealth::Unknown`] at the TUI layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StygianAvailability {
    /// Fallback is available and endpoint is healthy.
    Available { endpoint_url: String },
    /// Fallback is not available with diagnostic reason.
    Unavailable(StygianUnavailableReason),
}

/// Execution mode selected for a fetch operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchExecutionMode {
    /// Use direct HTTP/API path.
    NativeHttp,
    /// Use stygian-mcp fallback path.
    StygianMcpFallback,
    /// Source is JS-heavy but no fallback is available; caller should fail-soft.
    FailSoft,
}

/// Probe stygian-mcp availability using compile-time and runtime checks.
///
/// # Errors
///
/// Returns `Err` if the probe HTTP client cannot be built.
pub async fn detect_stygian_availability(
    config: &StygianProbeConfig,
) -> Result<StygianAvailability, FetchError> {
    detect_stygian_availability_with_compile_gate(config, cfg!(feature = "stygian")).await
}

async fn detect_stygian_availability_with_compile_gate(
    config: &StygianProbeConfig,
    compile_gate_enabled: bool,
) -> Result<StygianAvailability, FetchError> {
    if !compile_gate_enabled {
        return Ok(StygianAvailability::Unavailable(
            StygianUnavailableReason::FeatureDisabled,
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .build()
        .map_err(FetchError::Http)?;

    let mut last_unhealthy: Option<(u16, String)> = None;
    let mut last_error: Option<String> = None;
    let attempts = u16::from(config.retries) + 1;
    for _ in 0..attempts {
        match client.get(&config.endpoint_url).send().await {
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();

                if status.is_success() && body_looks_healthy(&body) {
                    return Ok(StygianAvailability::Available {
                        endpoint_url: config.endpoint_url.clone(),
                    });
                }

                // Unhealthy response — record it and retry (may be transient).
                last_unhealthy = Some((status.as_u16(), body));
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
    }

    if let Some((status, body)) = last_unhealthy {
        return Ok(StygianAvailability::Unavailable(
            StygianUnavailableReason::EndpointUnhealthy {
                endpoint_url: config.endpoint_url.clone(),
                status,
                body,
            },
        ));
    }

    Ok(StygianAvailability::Unavailable(
        StygianUnavailableReason::EndpointUnreachable {
            endpoint_url: config.endpoint_url.clone(),
            message: last_error.unwrap_or_else(|| "unknown probe error".to_string()),
        },
    ))
}

fn body_looks_healthy(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("\"ok\"")
        || lower.contains("\"healthy\"")
        || lower.contains("status:ok")
        || lower.trim() == "ok"
}

/// Select fetch execution mode based on source type and stygian availability.
#[must_use]
pub const fn select_execution_mode(
    is_js_heavy_source: bool,
    availability: &StygianAvailability,
) -> FetchExecutionMode {
    if !is_js_heavy_source {
        return FetchExecutionMode::NativeHttp;
    }

    match availability {
        StygianAvailability::Available { .. } => FetchExecutionMode::StygianMcpFallback,
        StygianAvailability::Unavailable(_) => FetchExecutionMode::FailSoft,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn spawn_health_server(status_code: u16, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 {status_code} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        format!("http://{addr}/health")
    }

    #[tokio::test]
    async fn detection_returns_unavailable_when_feature_disabled() {
        let cfg = StygianProbeConfig::default();
        let availability = detect_stygian_availability_with_compile_gate(&cfg, false)
            .await
            .unwrap();
        assert_eq!(
            availability,
            StygianAvailability::Unavailable(StygianUnavailableReason::FeatureDisabled)
        );
    }

    #[tokio::test]
    async fn detection_returns_unavailable_when_endpoint_health_check_fails() {
        let endpoint = "http://127.0.0.1:9/health".to_string();
        let cfg = StygianProbeConfig {
            endpoint_url: endpoint,
            timeout_ms: 100,
            retries: 0,
        };

        let availability = detect_stygian_availability_with_compile_gate(&cfg, true)
            .await
            .unwrap();
        match availability {
            StygianAvailability::Unavailable(StygianUnavailableReason::EndpointUnreachable {
                ..
            }) => {}
            other => panic!("expected endpoint unreachable, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn detection_returns_available_when_endpoint_reports_healthy() {
        let endpoint = spawn_health_server(200, "{\"status\":\"ok\"}");
        let cfg = StygianProbeConfig {
            endpoint_url: endpoint.clone(),
            timeout_ms: 500,
            retries: 0,
        };

        let availability = detect_stygian_availability_with_compile_gate(&cfg, true)
            .await
            .unwrap();
        assert_eq!(
            availability,
            StygianAvailability::Available {
                endpoint_url: endpoint
            }
        );
    }

    #[test]
    fn policy_selects_native_http_for_non_js_sources_even_when_available() {
        let availability = StygianAvailability::Available {
            endpoint_url: "http://127.0.0.1:8787/health".to_string(),
        };
        let mode = select_execution_mode(false, &availability);
        assert_eq!(mode, FetchExecutionMode::NativeHttp);
    }

    #[test]
    fn policy_selects_fallback_for_js_heavy_sources_when_available() {
        let availability = StygianAvailability::Available {
            endpoint_url: "http://127.0.0.1:8787/health".to_string(),
        };
        let mode = select_execution_mode(true, &availability);
        assert_eq!(mode, FetchExecutionMode::StygianMcpFallback);
    }
}
