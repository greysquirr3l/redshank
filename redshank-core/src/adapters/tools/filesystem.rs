//! Filesystem tools: `list_files`, `search_files`, `repo_map`, `read_file`, `write_file`,
//! `edit_file`, `hashline_edit`, `read_image`, `apply_patch`.

use std::fmt::Write as _;

use super::workspace_tools::WorkspaceTools;
use serde_json::Value;
use std::path::Path;

/// 2-char hex CRC hash of a line (whitespace-invariant).
fn line_hash(line: &str) -> String {
    let normalized: String = line.split_whitespace().collect();
    let crc = crc32fast::hash(normalized.as_bytes());
    format!("{:02x}", crc & 0xFF)
}

/// List files in the workspace.
pub async fn list_files(ws: &WorkspaceTools, args: &Value) -> String {
    let glob = args.get("glob").and_then(|v| v.as_str());

    // Try ripgrep first, fall back to walkdir
    let files = list_files_rg(&ws.root, glob, ws.command_timeout_secs)
        .await
        .unwrap_or_else(|| list_files_walk(&ws.root, glob, 50_000));

    if files.is_empty() {
        return "(no files)".to_string();
    }

    let clipped: Vec<&str> = files
        .iter()
        .map(String::as_str)
        .take(ws.max_files_listed)
        .collect();
    let mut output = clipped.join("\n");
    if files.len() > clipped.len() {
        let omitted = files.len() - clipped.len();
        let _ = write!(output, "\n...[omitted {omitted} files]...");
    }
    output
}

/// List files using ripgrep.
async fn list_files_rg(root: &Path, glob: Option<&str>, timeout_secs: u64) -> Option<Vec<String>> {
    if tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        return None;
    }

    let mut cmd = tokio::process::Command::new("rg");
    cmd.args(["--files", "--hidden", "-g", "!.git"])
        .current_dir(root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(g) = glob {
        cmd.args(["-g", g]);
    }

    let output = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), cmd.output())
        .await
        .ok()?
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<String> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(ToString::to_string)
        .collect();
    Some(lines)
}

/// List files using `std::fs` walk (fallback).
fn list_files_walk(root: &Path, glob: Option<&str>, max_entries: usize) -> Vec<String> {
    let mut results = Vec::new();
    let mut count = 0;
    walk_dir(root, root, glob, max_entries, &mut count, &mut results);
    results.sort_unstable();
    results
}

fn walk_dir(
    dir: &Path,
    root: &Path,
    glob: Option<&str>,
    max_entries: usize,
    count: &mut usize,
    results: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str == ".git" {
            continue;
        }

        if path.is_dir() {
            walk_dir(&path, root, glob, max_entries, count, results);
        } else {
            *count += 1;
            if *count > max_entries {
                return;
            }
            if let Ok(rel) = path.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().to_string();
                if let Some(g) = glob
                    && !fnmatch(&rel_str, g)
                {
                    continue;
                }
                results.push(rel_str);
            }
        }
    }
}

/// Simple glob matching (supports * and **).
fn fnmatch(path: &str, pattern: &str) -> bool {
    // Simple case: pattern is just an extension like "*.rs"
    if let Some(ext) = pattern.strip_prefix("*.") {
        return path.ends_with(&format!(".{ext}"));
    }
    if let Some(ext) = pattern.strip_prefix("**/*.") {
        return path.ends_with(&format!(".{ext}"));
    }
    path.contains(pattern)
}

/// Search file contents.
pub async fn search_files(ws: &WorkspaceTools, args: &Value) -> String {
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) if !q.trim().is_empty() => q,
        _ => return "query cannot be empty".to_string(),
    };
    let glob = args.get("glob").and_then(|v| v.as_str());

    // Try ripgrep first
    if let Some(result) = search_rg(
        &ws.root,
        query,
        glob,
        ws.max_search_hits,
        ws.command_timeout_secs,
    )
    .await
    {
        return result;
    }

    // Fallback: grep-like walk
    search_walk(&ws.root, query, glob, ws.max_search_hits)
}

async fn search_rg(
    root: &Path,
    query: &str,
    glob: Option<&str>,
    max_hits: usize,
    timeout_secs: u64,
) -> Option<String> {
    if tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        return None;
    }

    let mut cmd = tokio::process::Command::new("rg");
    cmd.args(["-n", "--hidden", "-S", query, "."])
        .current_dir(root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(g) = glob {
        cmd.args(["-g", g]);
    }

    let output = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), cmd.output())
        .await
        .ok()?
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Some("(no matches)".to_string());
    }
    let clipped: Vec<&str> = lines.into_iter().take(max_hits).collect();
    Some(clipped.join("\n"))
}

fn search_walk(root: &Path, query: &str, _glob: Option<&str>, max_hits: usize) -> String {
    fn walk_search(
        dir: &Path,
        root: &Path,
        lower_query: &str,
        max_hits: usize,
        matches: &mut Vec<String>,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            if matches.len() >= max_hits {
                return;
            }
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk_search(&path, root, lower_query, max_hits, matches);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                for (idx, line) in text.lines().enumerate() {
                    if matches.len() >= max_hits {
                        return;
                    }
                    if line.to_lowercase().contains(lower_query)
                        && let Ok(rel) = path.strip_prefix(root)
                    {
                        matches.push(format!("{}:{}:{}", rel.display(), idx + 1, line,));
                    }
                }
            }
        }
    }

    let lower_query = query.to_lowercase();
    let mut matches = Vec::new();
    walk_search(root, root, &lower_query, max_hits, &mut matches);
    if matches.is_empty() {
        "(no matches)".to_string()
    } else {
        matches.join("\n")
    }
}

/// Build a repo map of source file symbols.
pub async fn repo_map(ws: &WorkspaceTools, args: &Value) -> String {
    let glob = args.get("glob").and_then(|v| v.as_str());
    let max_files = args
        .get("max_files")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .clamp(1, 500) as usize;

    // Get file list
    let files = list_files_rg(&ws.root, glob, ws.command_timeout_secs)
        .await
        .unwrap_or_else(|| list_files_walk(&ws.root, glob, 50_000));
    let candidates: Vec<&str> = files.iter().map(String::as_str).take(max_files).collect();
    if candidates.is_empty() {
        return "(no files)".to_string();
    }

    let source_extensions = [
        ".py", ".js", ".jsx", ".ts", ".tsx", ".go", ".rs", ".java", ".c", ".h", ".cpp", ".hpp",
        ".cs", ".rb", ".php", ".swift", ".kt", ".scala", ".sh",
    ];

    let mut file_entries = Vec::new();
    for rel in &candidates {
        let ext = Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"));
        let ext_ref = ext.as_deref().unwrap_or("");
        if !source_extensions.contains(&ext_ref) {
            continue;
        }
        let full_path = ws.root.join(rel);
        if !full_path.exists() || full_path.is_dir() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&full_path) else {
            continue;
        };
        let symbols = extract_symbols(&text);
        let lines = text.lines().count();
        file_entries.push(serde_json::json!({
            "path": rel,
            "lines": lines,
            "symbols": symbols,
        }));
    }

    let output = serde_json::json!({
        "root": ws.root.display().to_string(),
        "files": file_entries,
        "total": file_entries.len(),
    });
    let json_str = serde_json::to_string_pretty(&output).unwrap_or_default();
    WorkspaceTools::clip(&json_str, ws.max_file_chars)
}

/// Extract generic symbols from source code (functions, classes).
fn extract_symbols(text: &str) -> Vec<serde_json::Value> {
    use regex::Regex;
    use std::sync::LazyLock;

    static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*[(<]")
            .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"))
    });
    static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:pub\s+)?(?:struct|enum|trait|class|interface)\s+(\w+)")
            .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"))
    });
    static JS_FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(")
            .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"))
    });
    static PY_FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^(?:async\s+)?def\s+(\w+)\s*\(")
            .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"))
    });
    static PY_CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^class\s+(\w+)")
            .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"))
    });

    let cap_line = |cap: &regex::Captures<'_>| {
        text.get(..cap.get(0).map_or(0, |m| m.start()))
            .map_or(0, |s| s.lines().count())
            + 1
    };

    let mut symbols = Vec::new();
    for cap in FUNC_RE.captures_iter(text) {
        let line = cap_line(&cap);
        symbols.push(serde_json::json!({"kind": "function", "name": &cap[1], "line": line}));
    }
    for cap in CLASS_RE.captures_iter(text) {
        let line = cap_line(&cap);
        symbols.push(serde_json::json!({"kind": "class", "name": &cap[1], "line": line}));
    }
    for cap in JS_FUNC_RE.captures_iter(text) {
        let line = cap_line(&cap);
        symbols.push(serde_json::json!({"kind": "function", "name": &cap[1], "line": line}));
    }
    for cap in PY_FUNC_RE.captures_iter(text) {
        let line = cap_line(&cap);
        symbols.push(serde_json::json!({"kind": "function", "name": &cap[1], "line": line}));
    }
    for cap in PY_CLASS_RE.captures_iter(text) {
        let line = cap_line(&cap);
        symbols.push(serde_json::json!({"kind": "class", "name": &cap[1], "line": line}));
    }
    symbols.sort_by_key(|s| {
        s.get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    });
    symbols.truncate(200);
    symbols
}

/// Read a file with hashline numbering.
pub async fn read_file(ws: &WorkspaceTools, args: &Value) -> String {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return "read_file requires 'path' parameter".to_string();
    };
    let hashline = args
        .get("hashline")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let resolved = match ws.resolve_path(path) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };

    if !resolved.exists() {
        return format!("File not found: {path}");
    }
    if resolved.is_dir() {
        return format!("Path is a directory, not a file: {path}");
    }

    let text = match std::fs::read_to_string(&resolved) {
        Ok(t) => t,
        Err(e) => return format!("Failed to read file {path}: {e}"),
    };

    ws.mark_read(&resolved).await;

    let clipped = WorkspaceTools::clip(&text, ws.max_file_chars);
    let rel = resolved
        .strip_prefix(&ws.root)
        .map_or_else(|_| path.to_string(), |p| p.to_string_lossy().to_string());

    let numbered = if hashline {
        clipped
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{}:{}|{}", i + 1, line_hash(line), line))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        clipped
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{}|{}", i + 1, line))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!("# {rel}\n{numbered}")
}

/// Read an image file, returning base64-encoded data.
#[allow(clippy::unused_async)]
pub async fn read_image(ws: &WorkspaceTools, args: &Value) -> String {
    use base64::Engine;
    const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return "read_image requires 'path' parameter".to_string();
    };

    let resolved = match ws.resolve_path(path) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };

    if !resolved.exists() {
        return format!("File not found: {path}");
    }
    if resolved.is_dir() {
        return format!("Path is a directory, not a file: {path}");
    }

    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();

    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => {
            return format!(
                "Unsupported image format: .{ext}. Supported: .gif, .jpeg, .jpg, .png, .webp"
            );
        }
    };

    match std::fs::metadata(&resolved) {
        Ok(meta) if meta.len() > MAX_IMAGE_BYTES => {
            return format!(
                "Image too large: {} bytes (max {} bytes)",
                meta.len(),
                MAX_IMAGE_BYTES
            );
        }
        Err(e) => return format!("Failed to read image {path}: {e}"),
        _ => {}
    }

    let raw = match std::fs::read(&resolved) {
        Ok(data) => data,
        Err(e) => return format!("Failed to read image {path}: {e}"),
    };

    let b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
    let rel = resolved
        .strip_prefix(&ws.root)
        .map_or_else(|_| path.to_string(), |p| p.to_string_lossy().to_string());

    // Return JSON with base64 data so the caller can build the image content block
    serde_json::json!({
        "text": format!("Image {rel} ({} bytes, {mime})", raw.len()),
        "base64": b64,
        "media_type": mime,
    })
    .to_string()
}

/// Write (create or overwrite) a file.
pub async fn write_file(ws: &WorkspaceTools, args: &Value) -> String {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return "write_file requires 'path' parameter".to_string();
    };
    let Some(content) = args.get("content").and_then(|v| v.as_str()) else {
        return "write_file requires 'content' parameter".to_string();
    };

    let resolved = match ws.resolve_path(path) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };

    // Pre-read guard
    if let Err(e) = ws.check_write_allowed(&resolved).await {
        return format!(
            "BLOCKED: {path} already exists but has not been read. \
             Use read_file('{path}') first. ({e})"
        );
    }

    // Parallel write registration
    if let Err(e) = ws.register_write_target(&resolved).await {
        return format!("BLOCKED: {e}");
    }

    // Create parent dirs
    if let Some(parent) = resolved.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return format!("Failed to create directory: {e}");
    }

    if let Err(e) = std::fs::write(&resolved, content) {
        return format!("Failed to write {path}: {e}");
    }

    ws.mark_read(&resolved).await;
    let rel = resolved
        .strip_prefix(&ws.root)
        .map_or_else(|_| path.to_string(), |p| p.to_string_lossy().to_string());
    format!("Wrote {} chars to {rel}", content.len())
}

/// Edit a file by replacing exact text (search-and-replace).
pub async fn edit_file(ws: &WorkspaceTools, args: &Value) -> String {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return "edit_file requires 'path' parameter".to_string();
    };
    let Some(old_text) = args.get("old_text").and_then(|v| v.as_str()) else {
        return "edit_file requires 'old_text' parameter".to_string();
    };
    let Some(new_text) = args.get("new_text").and_then(|v| v.as_str()) else {
        return "edit_file requires 'new_text' parameter".to_string();
    };

    let resolved = match ws.resolve_path(path) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };

    if !resolved.exists() {
        return format!("File not found: {path}");
    }
    if resolved.is_dir() {
        return format!("Path is a directory, not a file: {path}");
    }

    let content = match std::fs::read_to_string(&resolved) {
        Ok(t) => t,
        Err(e) => return format!("Failed to read file {path}: {e}"),
    };

    ws.mark_read(&resolved).await;

    // Exact match
    let new_content = if content.contains(old_text) {
        let count = content.matches(old_text).count();
        if count > 1 {
            return format!(
                "edit_file failed: old_text appears {count} times in {path}. \
                 Provide more context to make it unique."
            );
        }
        content.replacen(old_text, new_text, 1)
    } else {
        // Whitespace-normalized fallback
        let norm_old: Vec<&str> = old_text.split_whitespace().collect();
        let old_lines: Vec<&str> = old_text.lines().collect();
        let lines: Vec<&str> = content.lines().collect();

        let mut found = false;
        let mut new_content = String::new();

        for i in 0..=lines.len().saturating_sub(old_lines.len()) {
            let candidate: String = lines
                .get(i..i + old_lines.len())
                .unwrap_or_default()
                .join("\n");
            let norm_candidate: Vec<&str> = candidate.split_whitespace().collect();
            if norm_candidate == norm_old {
                let before: String = lines.get(..i).unwrap_or_default().join("\n");
                let after: String = lines
                    .get(i + old_lines.len()..)
                    .unwrap_or_default()
                    .join("\n");
                new_content = if before.is_empty() && after.is_empty() {
                    new_text.to_string()
                } else if before.is_empty() {
                    format!("{new_text}\n{after}")
                } else if after.is_empty() {
                    format!("{before}\n{new_text}")
                } else {
                    format!("{before}\n{new_text}\n{after}")
                };
                found = true;
                break;
            }
        }

        if !found {
            return format!("edit_file failed: old_text not found in {path}");
        }
        new_content
    };

    // Parallel write registration
    if let Err(e) = ws.register_write_target(&resolved).await {
        return format!("BLOCKED: {e}");
    }

    if let Err(e) = std::fs::write(&resolved, &new_content) {
        return format!("Failed to write {path}: {e}");
    }

    let rel = resolved
        .strip_prefix(&ws.root)
        .map_or_else(|_| path.to_string(), |p| p.to_string_lossy().to_string());
    format!("Edited {rel}")
}

/// Edit a file using hash-anchored line references.
#[allow(clippy::too_many_lines)]
pub async fn hashline_edit(ws: &WorkspaceTools, args: &Value) -> String {
    struct ParsedEdit {
        op: &'static str,
        start: usize,
        end: usize,
        new_lines: Vec<String>,
    }
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return "hashline_edit requires 'path' parameter".to_string();
    };
    let Some(edits) = args.get("edits").and_then(|v| v.as_array()) else {
        return "hashline_edit requires 'edits' array parameter".to_string();
    };

    let resolved = match ws.resolve_path(path) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };

    if !resolved.exists() {
        return format!("File not found: {path}");
    }
    if resolved.is_dir() {
        return format!("Path is a directory, not a file: {path}");
    }

    let content = match std::fs::read_to_string(&resolved) {
        Ok(t) => t,
        Err(e) => return format!("Failed to read file {path}: {e}"),
    };

    ws.mark_read(&resolved).await;

    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    let line_hashes: Vec<String> = lines.iter().map(|l| line_hash(l)).collect();
    let trailing_newline = content.ends_with('\n');

    // Parse and validate all edits
    let hashline_prefix = regex::Regex::new(r"^\d+:[0-9a-f]{2}\|")
        .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"));

    let mut parsed: Vec<ParsedEdit> = Vec::new();
    for edit in edits {
        if let Some(anchor) = edit.get("set_line").and_then(|v| v.as_str()) {
            let (lineno, err) = validate_anchor(anchor, &line_hashes, &lines);
            if let Some(e) = err {
                return e;
            }
            let raw = edit.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let clean = hashline_prefix.replace(raw, "").to_string();
            parsed.push(ParsedEdit {
                op: "set",
                start: lineno,
                end: lineno,
                new_lines: vec![clean],
            });
        } else if let Some(range) = edit.get("replace_lines") {
            let start_anchor = range.get("start").and_then(|v| v.as_str()).unwrap_or("");
            let end_anchor = range.get("end").and_then(|v| v.as_str()).unwrap_or("");
            let (start, err) = validate_anchor(start_anchor, &line_hashes, &lines);
            if let Some(e) = err {
                return e;
            }
            let (end, err) = validate_anchor(end_anchor, &line_hashes, &lines);
            if let Some(e) = err {
                return e;
            }
            if end < start {
                return format!("End line {end} is before start line {start}");
            }
            let raw = edit.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let new_lines: Vec<String> = raw
                .lines()
                .map(|l| hashline_prefix.replace(l, "").to_string())
                .collect();
            parsed.push(ParsedEdit {
                op: "replace",
                start,
                end,
                new_lines,
            });
        } else if let Some(anchor) = edit.get("insert_after").and_then(|v| v.as_str()) {
            let (lineno, err) = validate_anchor(anchor, &line_hashes, &lines);
            if let Some(e) = err {
                return e;
            }
            let raw = edit.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let new_lines: Vec<String> = raw
                .lines()
                .map(|l| hashline_prefix.replace(l, "").to_string())
                .collect();
            parsed.push(ParsedEdit {
                op: "insert",
                start: lineno,
                end: lineno,
                new_lines,
            });
        } else {
            return format!(
                "Unknown edit operation: {edit}. Use set_line, replace_lines, or insert_after."
            );
        }
    }

    // Sort bottom-up to preserve indices
    parsed.sort_by_key(|e| std::cmp::Reverse(e.start));

    let mut changed = 0;
    for edit in &parsed {
        match edit.op {
            "set" => {
                if let Some(line) = lines.get_mut(edit.start - 1)
                    && *line != edit.new_lines.first().map_or("", String::as_str)
                {
                    line.clone_from(edit.new_lines.first().unwrap_or(&String::new()));
                    changed += 1;
                }
            }
            "replace" => {
                let old: Vec<&str> = lines
                    .get(edit.start - 1..edit.end)
                    .unwrap_or_default()
                    .iter()
                    .map(String::as_str)
                    .collect();
                let new: Vec<&str> = edit.new_lines.iter().map(String::as_str).collect();
                if old != new {
                    lines.splice(edit.start - 1..edit.end, edit.new_lines.clone());
                    changed += 1;
                }
            }
            "insert" => {
                let insert_idx = edit.start; // insert after this line
                for (i, new_line) in edit.new_lines.iter().enumerate() {
                    lines.insert(insert_idx + i, new_line.clone());
                }
                changed += 1;
            }
            _ => {}
        }
    }

    if changed == 0 {
        return format!("No changes needed in {path}");
    }

    let mut new_content = lines.join("\n");
    if trailing_newline {
        new_content.push('\n');
    }

    // Parallel write registration
    if let Err(e) = ws.register_write_target(&resolved).await {
        return format!("BLOCKED: {e}");
    }

    if let Err(e) = std::fs::write(&resolved, &new_content) {
        return format!("Failed to write {path}: {e}");
    }

    let rel = resolved
        .strip_prefix(&ws.root)
        .map_or_else(|_| path.to_string(), |p| p.to_string_lossy().to_string());
    format!("Edited {rel} ({changed} edit(s) applied)")
}

/// Validate a hashline anchor "N:HH".
fn validate_anchor(
    anchor: &str,
    line_hashes: &[String],
    lines: &[String],
) -> (usize, Option<String>) {
    let parts: Vec<&str> = anchor.splitn(2, ':').collect();
    if parts.len() != 2 || parts.get(1).is_none_or(|s| s.len() != 2) {
        return (
            0,
            Some(format!("Invalid anchor format: '{anchor}' (expected N:HH)")),
        );
    }
    let lineno: usize = match parts.first().and_then(|s| s.parse().ok()) {
        Some(n) if n >= 1 => n,
        _ => {
            return (
                0,
                Some(format!("Invalid line number in anchor: '{anchor}'")),
            );
        }
    };
    if lineno > lines.len() {
        return (
            0,
            Some(format!(
                "Line {lineno} out of range (file has {} lines)",
                lines.len()
            )),
        );
    }
    let expected_hash = line_hashes.get(lineno - 1).map_or("", String::as_str);
    let anchor_hash = parts.get(1).copied().unwrap_or("");
    if expected_hash != anchor_hash {
        let ctx_start = lineno.saturating_sub(2).max(1);
        let ctx_end = (lineno + 2).min(lines.len());
        let ctx: Vec<String> = (ctx_start..=ctx_end)
            .map(|i| {
                format!(
                    "  {}:{}|{}",
                    i,
                    line_hashes.get(i - 1).map_or("", String::as_str),
                    lines.get(i - 1).map_or("", String::as_str),
                )
            })
            .collect();
        return (
            0,
            Some(format!(
                "Hash mismatch at line {lineno}: expected {anchor_hash}, got {expected_hash}. \
                 Current context:\n{}",
                ctx.join("\n")
            )),
        );
    }
    (lineno, None)
}

/// Apply a Codex-style patch.
pub async fn apply_patch(ws: &WorkspaceTools, args: &Value) -> String {
    let patch_text = match args.get("patch").and_then(|v| v.as_str()) {
        Some(p) if !p.trim().is_empty() => p,
        _ => return "apply_patch requires non-empty 'patch' parameter".to_string(),
    };

    let root = ws.root.clone();
    let resolve = |raw: &str| -> Result<std::path::PathBuf, String> {
        ws.resolve_path(raw).map_err(|e| e.to_string())
    };

    let report = super::patching::apply_patch(patch_text, &resolve);

    // Mark all added/updated files as read so subsequent edits are allowed
    for p in report.added.iter().chain(report.updated.iter()) {
        ws.mark_read(p).await;
    }
    // Also mark move destinations
    for (_, to) in &report.moved {
        ws.mark_read(to).await;
    }

    let _ = root; // suppress unused if needed
    report.render()
}
