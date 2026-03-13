//! Codex-style patch format parser and applier (T11).
//!
//! Format:
//! ```text
//! *** Begin Patch
//! *** Add File: <path>
//! +<content line 1>
//! +<content line 2>
//! *** Delete File: <path>
//! *** Update File: <path>
//! [*** Move to: <new_path>]
//!  <context line>
//! -<removed line>
//! +<added line>
//! *** End Patch
//! ```
//!
//! Two-pass hunk matching: first exact, then whitespace-normalised.

use std::fmt;
use std::path::PathBuf;

// ── Error ────────────────────────────────────────────────────────────────────

/// Errors arising from patch parsing or application.
#[derive(Debug)]
pub struct PatchError {
    pub message: String,
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PatchError {}

impl PatchError {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

// ── AST ──────────────────────────────────────────────────────────────────────

/// A single hunk of context / remove / add lines.
struct Chunk {
    lines: Vec<String>,
}

/// Add a new file.
struct AddFileOp {
    path: String,
    plus_lines: Vec<String>,
}

/// Delete an existing file.
struct DeleteFileOp {
    path: String,
}

/// Update an existing file (with optional rename).
struct UpdateFileOp {
    path: String,
    raw_lines: Vec<String>,
    move_to: Option<String>,
}

/// Parsed patch operation.
enum PatchOp {
    Add(AddFileOp),
    Delete(DeleteFileOp),
    Update(UpdateFileOp),
}

// ── Report ───────────────────────────────────────────────────────────────────

/// Summary of a patch application.
#[derive(Debug, Default)]
pub struct ApplyReport {
    pub added: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
    pub updated: Vec<PathBuf>,
    pub moved: Vec<(PathBuf, PathBuf)>,
    pub errors: Vec<PatchError>,
}

impl ApplyReport {
    /// Human-readable summary.
    pub fn render(&self) -> String {
        if !self.errors.is_empty() {
            let mut out = String::from("Patch partially applied.\nErrors:\n");
            for e in &self.errors {
                out.push_str(&format!("- {e}\n"));
            }
            if !self.added.is_empty() || !self.updated.is_empty() || !self.deleted.is_empty() {
                out.push_str("Succeeded:\n");
            }
            self.append_success(&mut out);
            return out;
        }
        let mut out = String::from("Patch applied successfully.\n");
        self.append_success(&mut out);
        out
    }

    fn append_success(&self, out: &mut String) {
        if !self.added.is_empty() {
            out.push_str("Added:\n");
            for p in &self.added {
                out.push_str(&format!("- {}\n", p.display()));
            }
        }
        if !self.updated.is_empty() {
            out.push_str("Updated:\n");
            for p in &self.updated {
                out.push_str(&format!("- {}\n", p.display()));
            }
        }
        if !self.deleted.is_empty() {
            out.push_str("Deleted:\n");
            for p in &self.deleted {
                out.push_str(&format!("- {}\n", p.display()));
            }
        }
        if !self.moved.is_empty() {
            out.push_str("Moved:\n");
            for (from, to) in &self.moved {
                out.push_str(&format!("- {} -> {}\n", from.display(), to.display()));
            }
        }
    }
}

// ── Parser ───────────────────────────────────────────────────────────────────

/// Parse a Codex-style patch string into operations.
fn parse_patch(patch_text: &str) -> Result<Vec<PatchOp>, PatchError> {
    let lines: Vec<&str> = patch_text.lines().collect();
    if lines.is_empty() {
        return Err(PatchError::new("patch is empty"));
    }
    if lines[0].trim() != "*** Begin Patch" {
        return Err(PatchError::new("patch must start with '*** Begin Patch'"));
    }
    if lines[lines.len() - 1].trim() != "*** End Patch" {
        return Err(PatchError::new("patch must end with '*** End Patch'"));
    }

    let mut ops = Vec::new();
    let mut i = 1;
    let last = lines.len() - 1;

    while i < last {
        let line = lines[i];

        if let Some(path) = line.strip_prefix("*** Add File: ") {
            let path = path.trim().to_string();
            i += 1;
            let mut plus_lines = Vec::new();
            while i < last && !lines[i].starts_with("*** ") {
                let row = lines[i];
                if let Some(content) = row.strip_prefix('+') {
                    plus_lines.push(content.to_string());
                } else {
                    return Err(PatchError::new(format!(
                        "add file '{path}' contains non '+' line: {row:?}"
                    )));
                }
                i += 1;
            }
            ops.push(PatchOp::Add(AddFileOp { path, plus_lines }));
            continue;
        }

        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            ops.push(PatchOp::Delete(DeleteFileOp {
                path: path.trim().to_string(),
            }));
            i += 1;
            continue;
        }

        if let Some(path) = line.strip_prefix("*** Update File: ") {
            let path = path.trim().to_string();
            i += 1;
            let mut move_to = None;
            if i < last
                && let Some(dest) = lines[i].strip_prefix("*** Move to: ")
            {
                move_to = Some(dest.trim().to_string());
                i += 1;
            }
            let mut raw_lines = Vec::new();
            while i < last && !lines[i].starts_with("*** ") {
                raw_lines.push(lines[i].to_string());
                i += 1;
            }
            ops.push(PatchOp::Update(UpdateFileOp {
                path,
                raw_lines,
                move_to,
            }));
            continue;
        }

        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        return Err(PatchError::new(format!(
            "unexpected patch line: {line:?}"
        )));
    }

    if ops.is_empty() {
        return Err(PatchError::new("patch contains no operations"));
    }
    Ok(ops)
}

// ── Chunk parser ─────────────────────────────────────────────────────────────

/// Parse raw hunk lines into chunks (split on `@@` separators).
fn parse_chunks(raw_lines: &[String]) -> Result<Vec<Chunk>, PatchError> {
    let mut chunks = Vec::new();
    let mut current: Vec<String> = Vec::new();

    for row in raw_lines {
        if row.starts_with("@@") {
            if !current.is_empty() {
                chunks.push(Chunk { lines: current });
                current = Vec::new();
            }
            continue;
        }
        if row == "*** End of File" {
            continue;
        }
        if row.starts_with(' ') || row.starts_with('+') || row.starts_with('-') {
            current.push(row.clone());
            continue;
        }
        return Err(PatchError::new(format!(
            "invalid update patch row: {row:?}"
        )));
    }
    if !current.is_empty() {
        chunks.push(Chunk { lines: current });
    }
    if chunks.is_empty() {
        return Err(PatchError::new(
            "update operation contains no hunks",
        ));
    }
    Ok(chunks)
}

/// Split a chunk into old (context + removed) and new (context + added) line sequences.
fn chunk_to_old_new(chunk: &Chunk) -> Result<(Vec<String>, Vec<String>), PatchError> {
    let mut old_seq = Vec::new();
    let mut new_seq = Vec::new();
    for row in &chunk.lines {
        let prefix = row.as_bytes().first().copied().unwrap_or(b' ');
        let payload = &row[1..];
        match prefix {
            b' ' => {
                old_seq.push(payload.to_string());
                new_seq.push(payload.to_string());
            }
            b'-' => {
                old_seq.push(payload.to_string());
            }
            b'+' => {
                new_seq.push(payload.to_string());
            }
            _ => {
                return Err(PatchError::new(format!(
                    "invalid row prefix: {:?}",
                    prefix as char
                )));
            }
        }
    }
    Ok((old_seq, new_seq))
}

// ── Matching ─────────────────────────────────────────────────────────────────

/// Collapse whitespace runs to a single space and strip.
fn normalize_ws(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Find `needle` in `haystack` starting from `start_idx`.
/// Pass 1: exact match.  Pass 2: whitespace-normalized match.
/// Returns the index or `None`.
fn find_subsequence(haystack: &[String], needle: &[String], start_idx: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start_idx.min(haystack.len()));
    }
    let max_start = haystack.len().checked_sub(needle.len())?;
    let start = start_idx.min(max_start);

    // Pass 1: exact
    for i in start..=max_start {
        if haystack[i..i + needle.len()] == *needle {
            return Some(i);
        }
    }

    // Pass 2: whitespace-normalised
    let norm_needle: Vec<String> = needle.iter().map(|l| normalize_ws(l)).collect();
    for i in start..=max_start {
        let norm_hay: Vec<String> = haystack[i..i + needle.len()]
            .iter()
            .map(|l| normalize_ws(l))
            .collect();
        if norm_hay == norm_needle {
            return Some(i);
        }
    }

    None
}

// ── Applier ──────────────────────────────────────────────────────────────────

/// Render line vec to string, optionally adding a trailing newline.
fn render_lines(lines: &[String], trailing_newline: bool) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut text = lines.join("\n");
    if trailing_newline {
        text.push('\n');
    }
    text
}

/// A path-resolution function: raw path → absolute PathBuf.
/// Returns Err if the path escapes the workspace.
type ResolveFn<'a> = &'a dyn Fn(&str) -> Result<PathBuf, String>;

/// Apply a parsed patch to the filesystem.
///
/// `resolve` turns a relative path from the patch into an absolute, validated
/// workspace path.  Returns an `ApplyReport` even on partial failure.
pub fn apply_patch(patch_text: &str, resolve: ResolveFn<'_>) -> ApplyReport {
    let mut report = ApplyReport::default();

    let ops = match parse_patch(patch_text) {
        Ok(ops) => ops,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };

    for op in ops {
        match op {
            PatchOp::Add(add) => match apply_add(&add, resolve) {
                Ok(path) => report.added.push(path),
                Err(e) => report.errors.push(e),
            },
            PatchOp::Delete(del) => match apply_delete(&del, resolve) {
                Ok(path) => report.deleted.push(path),
                Err(e) => report.errors.push(e),
            },
            PatchOp::Update(upd) => match apply_update(&upd, resolve) {
                Ok((path, moved)) => {
                    if let Some((from, to)) = moved {
                        report.moved.push((from, to));
                    }
                    report.updated.push(path);
                }
                Err(e) => report.errors.push(e),
            },
        }
    }

    report
}

fn apply_add(op: &AddFileOp, resolve: ResolveFn<'_>) -> Result<PathBuf, PatchError> {
    let target = resolve(&op.path).map_err(PatchError::new)?;
    if target.exists() {
        return Err(PatchError::new(format!(
            "cannot add existing file: {}",
            op.path
        )));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            PatchError::new(format!("failed to create directory: {e}"))
        })?;
    }
    let content = render_lines(&op.plus_lines, true);
    std::fs::write(&target, &content).map_err(|e| {
        PatchError::new(format!("failed to write {}: {e}", op.path))
    })?;
    Ok(target)
}

fn apply_delete(op: &DeleteFileOp, resolve: ResolveFn<'_>) -> Result<PathBuf, PatchError> {
    let target = resolve(&op.path).map_err(PatchError::new)?;
    if !target.exists() {
        return Err(PatchError::new(format!(
            "cannot delete missing file: {}",
            op.path
        )));
    }
    if target.is_dir() {
        return Err(PatchError::new(format!(
            "cannot delete directory with patch: {}",
            op.path
        )));
    }
    std::fs::remove_file(&target).map_err(|e| {
        PatchError::new(format!("failed to delete {}: {e}", op.path))
    })?;
    Ok(target)
}

fn apply_update(
    op: &UpdateFileOp,
    resolve: ResolveFn<'_>,
) -> Result<(PathBuf, Option<(PathBuf, PathBuf)>), PatchError> {
    let source = resolve(&op.path).map_err(PatchError::new)?;
    if !source.exists() {
        return Err(PatchError::new(format!(
            "cannot update missing file: {}",
            op.path
        )));
    }
    if source.is_dir() {
        return Err(PatchError::new(format!(
            "cannot update directory: {}",
            op.path
        )));
    }

    let original = std::fs::read_to_string(&source).map_err(|e| {
        PatchError::new(format!("failed to read {}: {e}", op.path))
    })?;
    let old_lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();
    let had_trailing_nl = original.ends_with('\n');

    let mut working = old_lines;
    let mut cursor: usize = 0;

    let chunks = parse_chunks(&op.raw_lines)?;
    for chunk in &chunks {
        let (old_seq, new_seq) = chunk_to_old_new(chunk)?;
        let idx = find_subsequence(&working, &old_seq, cursor)
            .or_else(|| find_subsequence(&working, &old_seq, 0));
        let idx = match idx {
            Some(i) => i,
            None => {
                let preview: String = old_seq
                    .iter()
                    .take(8)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(PatchError::new(format!(
                    "failed applying chunk to {}; could not locate:\n{preview}",
                    op.path
                )));
            }
        };
        let after: Vec<String> = working.split_off(idx + old_seq.len());
        working.truncate(idx);
        working.extend(new_seq);
        cursor = working.len();
        working.extend(after);
    }

    let output = render_lines(&working, had_trailing_nl);

    let mut moved_pair = None;
    let destination = if let Some(new_path) = &op.move_to {
        let dest = resolve(new_path).map_err(PatchError::new)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                PatchError::new(format!("failed to create directory: {e}"))
            })?;
        }
        std::fs::remove_file(&source).map_err(|e| {
            PatchError::new(format!("failed to remove source file during move: {e}"))
        })?;
        moved_pair = Some((source, dest.clone()));
        dest
    } else {
        source
    };

    std::fs::write(&destination, &output).map_err(|e| {
        PatchError::new(format!("failed to write {}: {e}", destination.display()))
    })?;

    Ok((destination, moved_pair))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_resolve(root: PathBuf) -> impl Fn(&str) -> Result<PathBuf, String> {
        move |raw: &str| {
            let p = root.join(raw);
            let resolved = if p.exists() {
                p.canonicalize().map_err(|e| e.to_string())?
            } else {
                if let Some(parent) = p.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                p
            };
            let canon_root = root.canonicalize().map_err(|e| e.to_string())?;
            if resolved.exists() && !resolved.starts_with(&canon_root) {
                return Err(format!("path escapes workspace: {raw}"));
            }
            Ok(resolved)
        }
    }

    #[test]
    fn add_file_creates_with_correct_content() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        let patch = "\
*** Begin Patch
*** Add File: hello.txt
+Hello, world!
+Second line.
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.added.len(), 1);

        let content = fs::read_to_string(root.join("hello.txt")).unwrap();
        assert_eq!(content, "Hello, world!\nSecond line.\n");
    }

    #[test]
    fn delete_file_removes_it() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        fs::write(root.join("doomed.txt"), "goodbye\n").unwrap();
        assert!(root.join("doomed.txt").exists());

        let patch = "\
*** Begin Patch
*** Delete File: doomed.txt
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.deleted.len(), 1);
        assert!(!root.join("doomed.txt").exists());
    }

    #[test]
    fn update_file_single_hunk() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        fs::write(
            root.join("code.rs"),
            "fn main() {\n    println!(\"old\");\n}\n",
        )
        .unwrap();

        let patch = "\
*** Begin Patch
*** Update File: code.rs
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
 }
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.updated.len(), 1);

        let content = fs::read_to_string(root.join("code.rs")).unwrap();
        assert!(content.contains("println!(\"new\")"));
        assert!(!content.contains("println!(\"old\")"));
    }

    #[test]
    fn update_file_whitespace_normalised_match() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        // File has extra spaces
        fs::write(
            root.join("spaces.rs"),
            "fn  foo()  {\n    println!( \"hi\" ) ;\n}\n",
        )
        .unwrap();

        // Patch uses normalised whitespace
        let patch = "\
*** Begin Patch
*** Update File: spaces.rs
 fn foo() {
-    println!( \"hi\" ) ;
+    println!(\"hello\");
 }
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let content = fs::read_to_string(root.join("spaces.rs")).unwrap();
        assert!(content.contains("println!(\"hello\")"));
    }

    #[test]
    fn multi_hunk_patch_applies_all() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        fs::write(
            root.join("multi.rs"),
            "fn a() {\n    old_a();\n}\n\nfn b() {\n    old_b();\n}\n",
        )
        .unwrap();

        let patch = "\
*** Begin Patch
*** Update File: multi.rs
 fn a() {
-    old_a();
+    new_a();
 }
@@
 fn b() {
-    old_b();
+    new_b();
 }
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);

        let content = fs::read_to_string(root.join("multi.rs")).unwrap();
        assert!(content.contains("new_a()"));
        assert!(content.contains("new_b()"));
        assert!(!content.contains("old_a()"));
        assert!(!content.contains("old_b()"));
    }

    #[test]
    fn path_outside_workspace_rejected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = move |raw: &str| -> Result<PathBuf, String> {
            let p = root.join(raw);
            // Simulated escape check
            if raw.contains("..") {
                return Err(format!("path escapes workspace: {raw}"));
            }
            Ok(p)
        };

        let patch = "\
*** Begin Patch
*** Add File: ../../../etc/passwd
+bad content
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert_eq!(report.errors.len(), 1);
        assert!(
            report.errors[0].message.contains("escapes workspace"),
            "got: {}",
            report.errors[0].message
        );
    }

    #[test]
    fn add_existing_file_is_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        fs::write(root.join("exists.txt"), "data\n").unwrap();

        let patch = "\
*** Begin Patch
*** Add File: exists.txt
+new data
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("existing file"));
    }

    #[test]
    fn delete_missing_file_is_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        let patch = "\
*** Begin Patch
*** Delete File: nosuchfile.txt
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("missing file"));
    }

    #[test]
    fn update_missing_file_is_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        let patch = "\
*** Begin Patch
*** Update File: ghost.rs
 fn phantom() {
-    a();
+    b();
 }
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("missing file"));
    }

    #[test]
    fn empty_patch_is_error() {
        let report = apply_patch("", &|_| Ok(PathBuf::from("x")));
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn missing_begin_marker_is_error() {
        let report = apply_patch("some random text\n*** End Patch", &|_| {
            Ok(PathBuf::from("x"))
        });
        assert!(!report.errors.is_empty());
        assert!(report.errors[0].message.contains("Begin Patch"));
    }

    #[test]
    fn missing_end_marker_is_error() {
        let report = apply_patch("*** Begin Patch\n*** Add File: x.txt\n+hi", &|_| {
            Ok(PathBuf::from("x"))
        });
        assert!(!report.errors.is_empty());
        assert!(report.errors[0].message.contains("End Patch"));
    }

    #[test]
    fn move_to_renames_file() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        fs::write(root.join("old_name.rs"), "fn main() {}\n").unwrap();

        let patch = "\
*** Begin Patch
*** Update File: old_name.rs
*** Move to: new_name.rs
 fn main() {}
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.moved.len(), 1);
        assert!(!root.join("old_name.rs").exists());
        assert!(root.join("new_name.rs").exists());
        let content = fs::read_to_string(root.join("new_name.rs")).unwrap();
        assert!(content.contains("fn main()"));
    }

    #[test]
    fn add_file_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        let patch = "\
*** Begin Patch
*** Add File: deep/nested/dir/file.txt
+content
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert!(root.join("deep/nested/dir/file.txt").exists());
    }

    #[test]
    fn chunk_not_found_returns_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        fs::write(root.join("mismatch.rs"), "fn real() {}\n").unwrap();

        let patch = "\
*** Begin Patch
*** Update File: mismatch.rs
 fn phantom_that_does_not_exist() {
-    a();
+    b();
 }
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("could not locate"));
    }

    #[test]
    fn partial_failure_reports_both_success_and_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        // First op will succeed, second will fail
        let patch = "\
*** Begin Patch
*** Add File: good.txt
+good content
*** Delete File: nonexistent.txt
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert_eq!(report.added.len(), 1);
        assert_eq!(report.errors.len(), 1);
        assert!(root.join("good.txt").exists());
    }

    #[test]
    fn preserves_trailing_newline() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        fs::write(root.join("trail.txt"), "line1\nline2\n").unwrap();

        let patch = "\
*** Begin Patch
*** Update File: trail.txt
-line1
+replaced1
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let content = fs::read_to_string(root.join("trail.txt")).unwrap();
        assert!(content.ends_with('\n'));
        assert!(content.contains("replaced1"));
    }

    #[test]
    fn no_trailing_newline_preserved() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let resolve = make_resolve(root.clone());

        // File without trailing newline
        fs::write(root.join("notrail.txt"), "line1\nline2").unwrap();

        let patch = "\
*** Begin Patch
*** Update File: notrail.txt
-line1
+replaced1
*** End Patch";

        let report = apply_patch(patch, &resolve);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let content = fs::read_to_string(root.join("notrail.txt")).unwrap();
        assert!(!content.ends_with('\n'));
        assert!(content.contains("replaced1"));
    }
}
