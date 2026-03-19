//! File-based JSONL replay logger with delta encoding.
//!
//! Mirrors `agent/replay_log.py` from the `OpenPlanter` Python implementation.
//! Each LLM API call is logged as a JSONL record so it can be replayed exactly.
//!
//! **Delta encoding**: seq 0 stores the full messages snapshot; seq N stores
//! only the messages appended since seq N-1. This dramatically reduces log
//! size on long conversations.
//!
//! **Hierarchical IDs**: child loggers for subtasks use `root/d{depth}s{step}`
//! notation, all appending to the same JSONL file.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use serde_json::Value;

use crate::domain::errors::DomainError;
use crate::ports::replay_log::ReplayLog;

/// Parameters for [`FileReplayLogger::write_header`].
pub struct HeaderParams<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub base_url: &'a str,
    pub system_prompt: &'a str,
    pub tool_defs: &'a [Value],
    pub reasoning_effort: Option<&'a str>,
    pub temperature: Option<f64>,
}

/// Parameters for [`FileReplayLogger::log_call`].
pub struct CallParams<'a> {
    pub depth: u32,
    pub step: u32,
    pub messages: &'a [Value],
    pub response: &'a Value,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub elapsed_sec: f64,
}

/// File-backed replay logger that appends JSONL records.
///
/// Thread-safe: uses atomics for the sequence counter and previous message
/// count so `&self` methods can be called from concurrent async contexts.
pub struct FileReplayLogger {
    /// Path to the JSONL log file.
    path: PathBuf,
    /// Hierarchical conversation ID (e.g. `"root"`, `"root/d1s3"`).
    conversation_id: String,
    /// Monotonically increasing sequence number for this conversation.
    seq: AtomicU32,
    /// Number of messages seen in the previous `log_call` invocation.
    prev_len: AtomicUsize,
}

impl FileReplayLogger {
    /// Create a root logger that writes to `path`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            conversation_id: "root".to_owned(),
            seq: AtomicU32::new(0),
            prev_len: AtomicUsize::new(0),
        }
    }

    /// Create a logger with a custom conversation ID.
    #[must_use]
    pub const fn with_conversation_id(path: PathBuf, conversation_id: String) -> Self {
        Self {
            path,
            conversation_id,
            seq: AtomicU32::new(0),
            prev_len: AtomicUsize::new(0),
        }
    }

    /// Create a child logger for a subtask.
    ///
    /// The child appends to the same file with ID `{parent}/d{depth}s{step}`.
    #[must_use]
    pub fn child(&self, depth: u32, step: u32) -> Self {
        let child_id = format!("{}/d{}s{}", self.conversation_id, depth, step);
        Self::with_conversation_id(self.path.clone(), child_id)
    }

    /// The hierarchical conversation ID.
    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    /// Path to the underlying JSONL file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write a session header record.
    ///
    /// Typically called once at the start of a conversation to record
    /// provider, model, system prompt, tool definitions, etc.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the log file cannot be written.
    pub fn write_header(&self, params: &HeaderParams<'_>) -> Result<(), DomainError> {
        let mut record = serde_json::json!({
            "type": "header",
            "conversation_id": self.conversation_id,
            "provider": params.provider,
            "model": params.model,
            "base_url": params.base_url,
            "system_prompt": params.system_prompt,
            "tool_defs": params.tool_defs,
        });
        if let Some(re) = params.reasoning_effort
            && let Some(obj) = record.as_object_mut()
        {
            obj.insert("reasoning_effort".to_owned(), Value::String(re.to_owned()));
        }
        if let Some(temp) = params.temperature
            && let Some(obj) = record.as_object_mut()
        {
            obj.insert("temperature".to_owned(), serde_json::json!(temp));
        }
        self.append_sync(&record)
    }

    /// Log an LLM API call with delta encoding.
    ///
    /// - `seq 0`: full `messages_snapshot` is stored.
    /// - `seq N` (N > 0): only `messages_delta` (new messages since the
    ///   previous call) is stored.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError`] if the log file cannot be written.
    pub fn log_call(&self, params: &CallParams<'_>) -> Result<(), DomainError> {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let prev = self.prev_len.swap(params.messages.len(), Ordering::Relaxed);

        let mut record = serde_json::json!({
            "type": "call",
            "conversation_id": self.conversation_id,
            "seq": seq,
            "depth": params.depth,
            "step": params.step,
            "ts": chrono::Utc::now().to_rfc3339(),
        });

        if let Some(obj) = record.as_object_mut() {
            if seq == 0 {
                obj.insert(
                    "messages_snapshot".to_owned(),
                    Value::Array(params.messages.to_vec()),
                );
            } else {
                let delta = params.messages.get(prev..).unwrap_or_default().to_vec();
                obj.insert("messages_delta".to_owned(), Value::Array(delta));
            }
            obj.insert("response".to_owned(), params.response.clone());
            obj.insert(
                "input_tokens".to_owned(),
                serde_json::json!(params.input_tokens),
            );
            obj.insert(
                "output_tokens".to_owned(),
                serde_json::json!(params.output_tokens),
            );
            obj.insert(
                "elapsed_sec".to_owned(),
                serde_json::json!((params.elapsed_sec * 1000.0).round() / 1000.0),
            );
        }

        self.append_sync(&record)
    }

    /// Append a single JSON record as one JSONL line.
    fn append_sync(&self, record: &Value) -> Result<(), DomainError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DomainError::Other(format!("create log dir: {e}")))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| DomainError::Other(format!("open replay log: {e}")))?;
        let line = serde_json::to_string(record)
            .map_err(|e| DomainError::Other(format!("serialize replay record: {e}")))?;
        writeln!(file, "{line}")
            .map_err(|e| DomainError::Other(format!("write replay log: {e}")))?;
        Ok(())
    }
}

impl ReplayLog for FileReplayLogger {
    async fn append(&self, record: &Value) -> Result<(), DomainError> {
        self.append_sync(record)
    }

    fn child_path(&self, subtask_id: &str) -> String {
        format!("{}/{}", self.conversation_id, subtask_id)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn read_lines(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn call<'a>(depth: u32, step: u32, msgs: &'a [Value], resp: &'a Value) -> CallParams<'a> {
        CallParams {
            depth,
            step,
            messages: msgs,
            response: resp,
            input_tokens: 0,
            output_tokens: 0,
            elapsed_sec: 0.0,
        }
    }

    #[test]
    fn seq0_stores_full_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");
        let logger = FileReplayLogger::new(log_path.clone());

        let msgs = vec![
            serde_json::json!({"role": "user", "content": "hello"}),
            serde_json::json!({"role": "assistant", "content": "hi"}),
        ];
        let resp = serde_json::json!({"content": "hi"});

        logger
            .log_call(&CallParams {
                depth: 0,
                step: 0,
                messages: &msgs,
                response: &resp,
                input_tokens: 10,
                output_tokens: 5,
                elapsed_sec: 0.123,
            })
            .unwrap();

        let lines = read_lines(&log_path);
        assert_eq!(lines.len(), 1);
        let rec = &lines[0];
        assert_eq!(rec["type"], "call");
        assert_eq!(rec["seq"], 0);
        assert!(rec.get("messages_snapshot").is_some());
        assert!(rec.get("messages_delta").is_none());
        assert_eq!(rec["messages_snapshot"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn seq1_stores_only_delta() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");
        let logger = FileReplayLogger::new(log_path.clone());

        let msgs_0 = vec![
            serde_json::json!({"role": "user", "content": "hello"}),
            serde_json::json!({"role": "assistant", "content": "hi"}),
        ];
        let resp_0 = serde_json::json!({"content": "hi"});
        logger
            .log_call(&CallParams {
                depth: 0,
                step: 0,
                messages: &msgs_0,
                response: &resp_0,
                input_tokens: 10,
                output_tokens: 5,
                elapsed_sec: 0.1,
            })
            .unwrap();

        // Two new messages appended.
        let msgs_1 = vec![
            serde_json::json!({"role": "user", "content": "hello"}),
            serde_json::json!({"role": "assistant", "content": "hi"}),
            serde_json::json!({"role": "user", "content": "how are you?"}),
            serde_json::json!({"role": "assistant", "content": "fine"}),
        ];
        let resp_1 = serde_json::json!({"content": "fine"});
        logger
            .log_call(&CallParams {
                depth: 0,
                step: 1,
                messages: &msgs_1,
                response: &resp_1,
                input_tokens: 20,
                output_tokens: 8,
                elapsed_sec: 0.25,
            })
            .unwrap();

        let lines = read_lines(&log_path);
        assert_eq!(lines.len(), 2);
        let rec = &lines[1];
        assert_eq!(rec["type"], "call");
        assert_eq!(rec["seq"], 1);
        assert!(rec.get("messages_snapshot").is_none());
        assert!(rec.get("messages_delta").is_some());
        // Delta should contain only the 2 new messages.
        let delta = rec["messages_delta"].as_array().unwrap();
        assert_eq!(delta.len(), 2);
        assert_eq!(delta[0]["content"], "how are you?");
    }

    #[test]
    fn child_logger_id_format() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");
        let root = FileReplayLogger::new(log_path);

        let child = root.child(2, 5);
        assert_eq!(child.conversation_id(), "root/d2s5");

        let grandchild = child.child(3, 1);
        assert_eq!(grandchild.conversation_id(), "root/d2s5/d3s1");
    }

    #[test]
    fn child_path_via_port_trait() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");
        let logger = FileReplayLogger::new(log_path);

        assert_eq!(logger.child_path("d2s5"), "root/d2s5");
    }

    #[test]
    fn jsonl_integrity() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");
        let logger = FileReplayLogger::new(log_path.clone());

        logger
            .write_header(&HeaderParams {
                provider: "anthropic",
                model: "claude-4",
                base_url: "https://api.anthropic.com",
                system_prompt: "You are a helper.",
                tool_defs: &[],
                reasoning_effort: None,
                temperature: None,
            })
            .unwrap();

        let msgs = vec![serde_json::json!({"role": "user", "content": "test"})];
        let resp = serde_json::json!({"content": "ok"});
        logger.log_call(&call(0, 0, &msgs, &resp)).unwrap();

        // Every line must parse independently.
        let lines = read_lines(&log_path);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["type"], "header");
        assert_eq!(lines[1]["type"], "call");
    }

    #[test]
    fn write_header_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");
        let logger = FileReplayLogger::new(log_path.clone());

        logger
            .write_header(&HeaderParams {
                provider: "openai",
                model: "gpt-4.1",
                base_url: "https://api.openai.com",
                system_prompt: "sys",
                tool_defs: &[serde_json::json!({"name": "search"})],
                reasoning_effort: Some("high"),
                temperature: Some(0.7),
            })
            .unwrap();

        let lines = read_lines(&log_path);
        assert_eq!(lines[0]["reasoning_effort"], "high");
        assert_eq!(lines[0]["temperature"], 0.7);
        assert_eq!(lines[0]["tool_defs"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn append_mode_never_truncates() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");

        let msgs = vec![serde_json::json!({"role": "user", "content": "a"})];
        let resp = serde_json::json!({"content": "b"});

        // First logger session.
        let logger1 = FileReplayLogger::new(log_path.clone());
        logger1.log_call(&call(0, 0, &msgs, &resp)).unwrap();

        // Second logger appends to the same file.
        let logger2 = FileReplayLogger::new(log_path.clone());
        logger2.log_call(&call(0, 0, &msgs, &resp)).unwrap();

        let lines = read_lines(&log_path);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn elapsed_sec_rounded_to_millis() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("replay.jsonl");
        let logger = FileReplayLogger::new(log_path.clone());

        let msgs = vec![serde_json::json!({"role": "user", "content": "x"})];
        let resp = serde_json::json!({"ok": true});
        logger
            .log_call(&CallParams {
                depth: 0,
                step: 0,
                messages: &msgs,
                response: &resp,
                input_tokens: 0,
                output_tokens: 0,
                elapsed_sec: 1.234_567_89,
            })
            .unwrap();

        let lines = read_lines(&log_path);
        assert_eq!(lines[0]["elapsed_sec"], 1.235);
    }

    #[test]
    fn creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = dir.path().join("deep").join("nested").join("replay.jsonl");
        let logger = FileReplayLogger::new(log_path.clone());

        let msgs = vec![serde_json::json!({"role": "user", "content": "x"})];
        let resp = serde_json::json!({"ok": true});
        logger.log_call(&call(0, 0, &msgs, &resp)).unwrap();

        assert!(log_path.exists());
    }
}
