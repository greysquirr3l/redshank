//! Web tools: web_search (Exa API) and fetch_url.

use super::workspace_tools::WorkspaceTools;
use serde_json::{json, Value};

/// Search the web via Exa API.
pub async fn web_search(ws: &WorkspaceTools, args: &Value) -> String {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.trim().is_empty() => q.trim(),
        _ => return "web_search requires non-empty 'query' parameter".to_string(),
    };
    let num_results = args
        .get("num_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(10)
        .clamp(1, 20) as usize;
    let include_text = args
        .get("include_text")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let exa_key = match &ws.creds.exa_api_key {
        Some(key) if !key.is_empty() => key.clone(),
        _ => return "Web search failed: EXA_API_KEY not configured".to_string(),
    };

    let mut payload = json!({
        "query": query,
        "numResults": num_results,
    });
    if include_text {
        payload["contents"] = json!({"text": {"maxCharacters": 4000}});
    }

    let client = reqwest::Client::new();
    let response = match client
        .post("https://api.exa.ai/search")
        .header("x-api-key", exa_key.as_str())
        .header("Content-Type", "application/json")
        .header("User-Agent", "redshank/0.1.0")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Web search failed: {e}"),
    };

    let status = response.status();
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => return format!("Web search failed: could not read response: {e}"),
    };

    if !status.is_success() {
        return format!("Web search failed: HTTP {status}: {body}");
    }

    let parsed: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return format!("Web search failed: invalid JSON: {e}"),
    };

    let results = parsed
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let out_results: Vec<Value> = results
        .iter()
        .filter_map(|row| {
            let row = row.as_object()?;
            let mut item = json!({
                "url": row.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                "title": row.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "snippet": row.get("highlight")
                    .or_else(|| row.get("snippet"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            });
            if include_text
                && let Some(text) = row.get("text").and_then(|v| v.as_str())
            {
                item["text"] = Value::String(WorkspaceTools::clip(text, 4000));
            }
            Some(item)
        })
        .collect();

    let output = json!({
        "query": query,
        "results": out_results,
        "total": out_results.len(),
    });

    let json_str = serde_json::to_string_pretty(&output).unwrap_or_default();
    WorkspaceTools::clip(&json_str, ws.max_file_chars)
}

/// Fetch URL contents via Exa contents API.
pub async fn fetch_url(ws: &WorkspaceTools, args: &Value) -> String {
    let urls = match args.get("urls").and_then(|v| v.as_array()) {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .take(10)
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        None => return "fetch_url requires 'urls' array parameter".to_string(),
    };

    if urls.is_empty() {
        return "fetch_url requires at least one valid URL".to_string();
    }

    let exa_key = match &ws.creds.exa_api_key {
        Some(key) if !key.is_empty() => key.clone(),
        _ => return "Fetch URL failed: EXA_API_KEY not configured".to_string(),
    };

    let payload = json!({
        "ids": urls,
        "text": {"maxCharacters": 8000},
    });

    let client = reqwest::Client::new();
    let response = match client
        .post("https://api.exa.ai/contents")
        .header("x-api-key", exa_key.as_str())
        .header("Content-Type", "application/json")
        .header("User-Agent", "redshank/0.1.0")
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Fetch URL failed: {e}"),
    };

    let status = response.status();
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => return format!("Fetch URL failed: could not read response: {e}"),
    };

    if !status.is_success() {
        return format!("Fetch URL failed: HTTP {status}: {body}");
    }

    let parsed: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return format!("Fetch URL failed: invalid JSON: {e}"),
    };

    let results = parsed
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let pages: Vec<Value> = results
        .iter()
        .filter_map(|row| {
            let row = row.as_object()?;
            Some(json!({
                "url": row.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                "title": row.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "text": WorkspaceTools::clip(
                    row.get("text").and_then(|v| v.as_str()).unwrap_or(""),
                    8000,
                ),
            }))
        })
        .collect();

    let output = json!({
        "pages": pages,
        "total": pages.len(),
    });

    let json_str = serde_json::to_string_pretty(&output).unwrap_or_default();
    WorkspaceTools::clip(&json_str, ws.max_file_chars)
}
