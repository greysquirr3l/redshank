//! Shell tools: run_shell, run_shell_bg, check_shell_bg, kill_shell_bg, cleanup_bg_jobs.

use super::workspace_tools::WorkspaceTools;
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;
use tokio::process::Command;

/// A background shell job.
pub struct BgJob {
    pub child: tokio::process::Child,
    pub output_path: std::path::PathBuf,
}

static HEREDOC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<<-?\s*['"]?\w+['"]?"#).unwrap());

static INTERACTIVE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[;&|]\s*)(vim|nano|less|more|top|htop|man)\b").unwrap());

/// Check shell command against policy (heredoc, interactive).
fn check_shell_policy(command: &str) -> Option<String> {
    if HEREDOC_RE.is_match(command) {
        return Some(
            "BLOCKED: Heredoc syntax (<< EOF) is not allowed by runtime policy. \
             Use write_file/apply_patch for multi-line content."
                .to_string(),
        );
    }
    if INTERACTIVE_RE.is_match(command) {
        let prog = INTERACTIVE_RE
            .captures(command)
            .and_then(|c| c.get(2))
            .map(|m| m.as_str())
            .unwrap_or("unknown");
        return Some(format!(
            "BLOCKED: Interactive terminal program '{prog}' is not allowed by runtime policy."
        ));
    }
    None
}

/// Execute a shell command synchronously.
pub async fn run_shell(ws: &WorkspaceTools, args: &Value) -> String {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return "run_shell requires 'command' parameter".to_string(),
    };

    if let Some(blocked) = check_shell_policy(command) {
        return blocked;
    }

    let timeout_secs = args
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(ws.command_timeout_secs)
        .clamp(1, 600);

    let child = match Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(&ws.root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0) // start_new_session equivalent
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return format!("$ {command}\n[failed to start: {e}]"),
    };

    let timeout = std::time::Duration::from_secs(timeout_secs);

    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => return format!("$ {command}\n[error: {e}]"),
        Err(_) => {
            // Timeout — we can't use child here since wait_with_output consumed it,
            // but timeout means the future was dropped which drops child.
            return format!("$ {command}\n[timeout after {timeout_secs}s — processes killed]");
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(-1);
    let merged = format!(
        "$ {command}\n[exit_code={code}]\n[stdout]\n{stdout}\n[stderr]\n{stderr}"
    );
    WorkspaceTools::clip(&merged, ws.max_shell_output_chars)
}

/// Start a background shell job.
pub async fn run_shell_bg(ws: &WorkspaceTools, args: &Value) -> String {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return "run_shell_bg requires 'command' parameter".to_string(),
    };

    if let Some(blocked) = check_shell_policy(command) {
        return blocked;
    }

    let mut next_id = ws.bg_next_id.lock().await;
    let job_id = *next_id;
    *next_id += 1;
    drop(next_id);

    let out_path = std::env::temp_dir().join(format!(".redshank_bg_{job_id}.out"));
    let out_file = match std::fs::File::create(&out_path) {
        Ok(f) => f,
        Err(e) => return format!("Failed to create output file: {e}"),
    };

    let child = match Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(&ws.root)
        .stdout(out_file.try_clone().unwrap_or(out_file))
        .stderr(std::process::Stdio::null())
        .process_group(0)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(&out_path);
            return format!("Failed to start background command: {e}");
        }
    };

    let pid = child.id().unwrap_or(0);
    let mut jobs = ws.bg_jobs.lock().await;
    jobs.insert(
        job_id,
        BgJob {
            child,
            output_path: out_path,
        },
    );

    format!("Background job started: job_id={job_id}, pid={pid}")
}

/// Check on a background job.
pub async fn check_shell_bg(ws: &WorkspaceTools, args: &Value) -> String {
    let job_id = match args.get("job_id").and_then(|v| v.as_u64()) {
        Some(id) => id as u32,
        None => return "check_shell_bg requires 'job_id' parameter".to_string(),
    };

    let mut jobs = ws.bg_jobs.lock().await;
    let job = match jobs.get_mut(&job_id) {
        Some(j) => j,
        None => return format!("No background job with id {job_id}"),
    };

    let output = std::fs::read_to_string(&job.output_path).unwrap_or_default();
    let clipped = WorkspaceTools::clip(&output, ws.max_shell_output_chars);

    match job.child.try_wait() {
        Ok(Some(status)) => {
            let code = status.code().unwrap_or(-1);
            let out_path = job.output_path.clone();
            jobs.remove(&job_id);
            let _ = std::fs::remove_file(&out_path);
            format!("[job {job_id} finished, exit_code={code}]\n{clipped}")
        }
        Ok(None) => {
            let pid = job.child.id().unwrap_or(0);
            format!("[job {job_id} still running, pid={pid}]\n{clipped}")
        }
        Err(e) => format!("[job {job_id} status error: {e}]\n{clipped}"),
    }
}

/// Kill a background job.
pub async fn kill_shell_bg(ws: &WorkspaceTools, args: &Value) -> String {
    let job_id = match args.get("job_id").and_then(|v| v.as_u64()) {
        Some(id) => id as u32,
        None => return "kill_shell_bg requires 'job_id' parameter".to_string(),
    };

    let mut jobs = ws.bg_jobs.lock().await;
    let mut job = match jobs.remove(&job_id) {
        Some(j) => j,
        None => return format!("No background job with id {job_id}"),
    };

    let _ = job.child.kill().await;
    let _ = job.child.wait().await;
    let _ = std::fs::remove_file(&job.output_path);

    format!("Background job {job_id} killed.")
}

/// Kill all background jobs.
pub async fn cleanup_bg_jobs(ws: &WorkspaceTools) -> String {
    let mut jobs = ws.bg_jobs.lock().await;
    let count = jobs.len();

    for (_, mut job) in jobs.drain() {
        let _ = job.child.kill().await;
        let _ = job.child.wait().await;
        let _ = std::fs::remove_file(&job.output_path);
    }

    format!("All {count} background job(s) cleaned up and killed.")
}
