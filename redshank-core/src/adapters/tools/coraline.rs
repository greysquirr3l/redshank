//! Coraline MCP tool bindings for self-directed code navigation.
//!
//! When the `coraline` feature is enabled, four additional tools become
//! available to the agent: `coraline_read_file`, `coraline_search`,
//! `coraline_repo_map`, and `coraline_edit_file`.
//!
//! These tools proxy to a Coraline MCP server over stdio.

use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use super::workspace_tools::WorkspaceTools;

/// Call the Coraline MCP server with a tool invocation.
///
/// Spawns a coraline process, sends the tool call over stdin as JSON-RPC,
/// and reads the response from stdout.
async fn mcp_call(workspace: &Path, tool_name: &str, arguments: &Value) -> String {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        }
    });

    let result = Command::new("coraline")
        .arg("mcp")
        .arg("--project-root")
        .arg(workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut child = match result {
        Ok(c) => c,
        Err(e) => return format!("Failed to spawn coraline MCP server: {e}"),
    };

    // Write request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_string(&request).unwrap_or_default();
        if let Err(e) = stdin.write_all(payload.as_bytes()).await {
            return format!("Failed to write to coraline stdin: {e}");
        }
        if let Err(e) = stdin.write_all(b"\n").await {
            return format!("Failed to write newline to coraline stdin: {e}");
        }
        drop(stdin); // Close stdin to signal EOF
    }

    // Read response from stdout
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => "Coraline MCP server returned empty response".to_string(),
            Ok(_) => parse_mcp_response(&line),
            Err(e) => format!("Failed to read from coraline stdout: {e}"),
        }
    } else {
        "Failed to capture coraline stdout".to_string()
    }
}

/// Parse a JSON-RPC response, extracting the result content.
fn parse_mcp_response(raw: &str) -> String {
    let Ok(resp) = serde_json::from_str::<Value>(raw) else {
        return format!("Invalid JSON from coraline: {}", &raw[..raw.len().min(200)]);
    };

    // Check for error
    if let Some(error) = resp.get("error") {
        let msg = error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return format!("Coraline error: {msg}");
    }

    // Extract result content
    if let Some(result) = resp.get("result") {
        if let Some(arr) = result.get("content").and_then(|c| c.as_array()) {
            return arr
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n");
        }
        // Fallback: stringify the result
        return serde_json::to_string_pretty(result).unwrap_or_default();
    }

    "No result in coraline response".to_string()
}

/// Read a file via Coraline's code-aware reader.
pub(super) async fn coraline_read_file(tools: &WorkspaceTools, arguments: &Value) -> String {
    let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return "Missing required parameter: path".to_string();
    }
    mcp_call(
        &tools.root,
        "coraline_read_file",
        &serde_json::json!({"path": path}),
    )
    .await
}

/// Search the codebase via Coraline's semantic search.
pub(super) async fn coraline_search(tools: &WorkspaceTools, arguments: &Value) -> String {
    let query = arguments
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if query.is_empty() {
        return "Missing required parameter: query".to_string();
    }
    let max_results = arguments
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(10);
    mcp_call(
        &tools.root,
        "coraline_search",
        &serde_json::json!({"query": query, "max_results": max_results}),
    )
    .await
}

/// Get a repository map (file tree with symbols) via Coraline.
pub(super) async fn coraline_repo_map(tools: &WorkspaceTools, arguments: &Value) -> String {
    let max_depth = arguments
        .get("max_depth")
        .and_then(Value::as_u64)
        .unwrap_or(3);
    mcp_call(
        &tools.root,
        "coraline_repo_map",
        &serde_json::json!({"max_depth": max_depth}),
    )
    .await
}

/// Edit a file via Coraline's code-aware editor.
pub(super) async fn coraline_edit_file(tools: &WorkspaceTools, arguments: &Value) -> String {
    let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let old_str = arguments
        .get("old_str")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new_str = arguments
        .get("new_str")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if path.is_empty() || old_str.is_empty() {
        return "Missing required parameters: path, old_str".to_string();
    }
    mcp_call(
        &tools.root,
        "coraline_edit_file",
        &serde_json::json!({"path": path, "old_str": old_str, "new_str": new_str}),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mcp_response_extracts_text_content() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"Hello world"}]}}"#;
        assert_eq!(parse_mcp_response(raw), "Hello world");
    }

    #[test]
    fn parse_mcp_response_handles_error() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-1,"message":"file not found"}}"#;
        let result = parse_mcp_response(raw);
        assert!(result.contains("file not found"));
    }

    #[test]
    fn parse_mcp_response_handles_invalid_json() {
        let result = parse_mcp_response("not json at all");
        assert!(result.contains("Invalid JSON"));
    }

    #[test]
    fn parse_mcp_response_handles_empty_result() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let result = parse_mcp_response(raw);
        assert!(result.contains("{}") || result.is_empty() || result.contains("No result"));
    }

    #[test]
    fn parse_mcp_response_combines_multiple_content_blocks() {
        let raw = r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"line1"},{"type":"text","text":"line2"}]}}"#;
        let result = parse_mcp_response(raw);
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
    }
}
