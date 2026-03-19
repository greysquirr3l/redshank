//! Provider-neutral tool definitions and provider-specific converters.
//!
//! Single source of truth for the tool schemas the agent can use.
//! Converter helpers produce the shapes expected by `OpenAI` and `Anthropic` APIs.

use crate::ports::model_provider::ToolDefinition;
use serde_json::json;

// ── Static Tool Definitions ─────────────────────────────────

/// All tool definitions available to the agent.
///
/// 20 entries when `recursive=true` (includes `subtask` and `execute`).
/// 18 entries when `recursive=false` (excludes them).
/// +4 when the `coraline` feature is enabled.
#[must_use]
pub fn tool_definitions(recursive: bool) -> Vec<ToolDefinition> {
    let mut defs = base_tools();
    if recursive {
        defs.extend(delegation_tools());
    }
    #[cfg(feature = "coraline")]
    defs.extend(coraline_tools());
    defs
}

/// The 18 base tools (always available).
// Justification: each tool definition is a data block; extracting individual
// definitions into helpers would not improve readability.
#[allow(clippy::too_many_lines)]
fn base_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_files".into(),
            description: "List files in the workspace directory. Optionally filter with a glob pattern.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "glob": {
                        "type": "string",
                        "description": "Optional glob pattern to filter files."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "search_files".into(),
            description: "Search file contents in the workspace for a text or regex query.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Text or regex to search for."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Optional glob pattern to restrict which files are searched."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "repo_map".into(),
            description: "Build a lightweight map of source files and symbols to speed up code navigation.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "glob": {
                        "type": "string",
                        "description": "Optional glob pattern to limit which files are scanned."
                    },
                    "max_files": {
                        "type": "integer",
                        "description": "Maximum number of files to scan (1-500, default 200)."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "read_file".into(),
            description: "Read the contents of a file in the workspace. Lines are numbered LINE:HASH|content by default for use with hashline_edit. Set hashline=false for plain N|content.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative or absolute path within the workspace."
                    },
                    "hashline": {
                        "type": "boolean",
                        "description": "Prefix each line with LINE:HASH| format for content verification. Default true."
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "read_image".into(),
            description: "Read an image file and return it for visual analysis. Supports PNG, JPEG, GIF, WebP.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative or absolute path to the image file within the workspace."
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "write_file".into(),
            description: "Create or overwrite a file in the workspace with the given content.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path for the file."
                    },
                    "content": {
                        "type": "string",
                        "description": "Full file content to write."
                    }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "edit_file".into(),
            description: "Replace a specific text span in a file. Provide the exact old text to find and the new text to replace it with. The old text must appear exactly once in the file.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to edit."
                    },
                    "old_text": {
                        "type": "string",
                        "description": "The exact text to find and replace."
                    },
                    "new_text": {
                        "type": "string",
                        "description": "The replacement text."
                    }
                },
                "required": ["path", "old_text", "new_text"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "apply_patch".into(),
            description: "Apply a Codex-style patch to one or more files. Use the *** Begin Patch / *** End Patch format with Update File, Add File, and Delete File operations.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "patch": {
                        "type": "string",
                        "description": "The full patch block in Codex patch format."
                    }
                },
                "required": ["patch"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "hashline_edit".into(),
            description: "Edit a file using hash-anchored line references from read_file(hashline=true). Operations: set_line (replace one line), replace_lines (replace a range), insert_after (insert new lines after an anchor).".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file."
                    },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "set_line": {
                                    "type": "string",
                                    "description": "Anchor 'N:HH' for single-line replace."
                                },
                                "replace_lines": {
                                    "type": "object",
                                    "description": "Range with 'start' and 'end' anchors.",
                                    "properties": {
                                        "start": { "type": "string" },
                                        "end": { "type": "string" }
                                    },
                                    "required": ["start", "end"],
                                    "additionalProperties": false
                                },
                                "insert_after": {
                                    "type": "string",
                                    "description": "Anchor 'N:HH' to insert after."
                                },
                                "content": {
                                    "type": "string",
                                    "description": "New content for the operation."
                                }
                            },
                            "required": [],
                            "additionalProperties": false
                        },
                        "description": "Edit operations: set_line, replace_lines, or insert_after."
                    }
                },
                "required": ["path", "edits"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "run_shell".into(),
            description: "Execute a shell command from the workspace root and return its output.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute."
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds for this command (default: agent default, max: 600)."
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "run_shell_bg".into(),
            description: "Start a shell command in the background. Returns a job ID to check or kill later.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to run in the background."
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "check_shell_bg".into(),
            description: "Check the status and output of a background job started with run_shell_bg.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "job_id": {
                        "type": "integer",
                        "description": "The job ID returned by run_shell_bg."
                    }
                },
                "required": ["job_id"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "kill_shell_bg".into(),
            description: "Kill a background job started with run_shell_bg.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "job_id": {
                        "type": "integer",
                        "description": "The job ID returned by run_shell_bg."
                    }
                },
                "required": ["job_id"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "cleanup_bg_jobs".into(),
            description: "Kill all remaining background jobs and return their final output.".into(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "web_search".into(),
            description: "Search the web using the Exa API. Returns URLs, titles, and optional page text.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Web search query string."
                    },
                    "num_results": {
                        "type": "integer",
                        "description": "Number of results to return (1-20, default 10)."
                    },
                    "include_text": {
                        "type": "boolean",
                        "description": "Whether to include page text in results."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "fetch_url".into(),
            description: "Fetch and return the text content of one or more URLs.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "urls": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of URLs to fetch."
                    }
                },
                "required": ["urls"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "begin_parallel_write_group".into(),
            description: "Begin a parallel write group. All write_file and edit_file calls after this are batched. Commit them atomically with end_parallel_write_group.".into(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "end_parallel_write_group".into(),
            description: "End the current parallel write group and apply all batched writes atomically.".into(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        },
    ]
}

/// Delegation tools: subtask and execute (only in recursive mode).
fn delegation_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "subtask".into(),
            description: "Spawn a recursive sub-agent to solve a smaller sub-problem. The result is returned as an observation.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "objective": {
                        "type": "string",
                        "description": "Clear objective for the sub-agent to accomplish."
                    },
                    "model": {
                        "type": "string",
                        "description": "Optional model for subtask (e.g. 'claude-sonnet-4-5-20250929')."
                    },
                    "reasoning_effort": {
                        "type": "string",
                        "enum": ["high", "medium", "low"],
                        "description": "Optional reasoning effort for the subtask model."
                    },
                    "acceptance_criteria": {
                        "type": "string",
                        "description": "Acceptance criteria for judging the subtask result."
                    }
                },
                "required": ["objective", "acceptance_criteria"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "execute".into(),
            description: "Hand an atomic sub-problem to a leaf executor agent with full tool access. Use this when the sub-problem requires no further decomposition and can be solved directly. The executor has no subtask or execute tools.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "objective": {
                        "type": "string",
                        "description": "Clear, specific objective for the executor to accomplish."
                    },
                    "acceptance_criteria": {
                        "type": "string",
                        "description": "Acceptance criteria for judging the executor result."
                    }
                },
                "required": ["objective", "acceptance_criteria"],
                "additionalProperties": false
            }),
        },
    ]
}

// ── Coraline MCP tools (behind feature flag) ────────────────

/// Four Coraline MCP tools for self-directed code navigation.
#[cfg(feature = "coraline")]
fn coraline_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "coraline_read_file".into(),
            description: "Read a file through Coraline's code-aware reader, which provides AST context and symbol information.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to read."
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "coraline_search".into(),
            description: "Search the codebase using Coraline's semantic code search. Returns relevant code snippets with file paths and line numbers.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language or code query to search for."
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 10)."
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "coraline_repo_map".into(),
            description: "Get a repository map showing the file tree with symbols (functions, classes, types) extracted by Coraline.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum directory depth to traverse (default: 3)."
                    }
                },
                "required": [],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "coraline_edit_file".into(),
            description: "Edit a file through Coraline's code-aware editor, which validates edits against the AST.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to edit."
                    },
                    "old_str": {
                        "type": "string",
                        "description": "Exact string to find and replace."
                    },
                    "new_str": {
                        "type": "string",
                        "description": "Replacement string."
                    }
                },
                "required": ["path", "old_str", "new_str"],
                "additionalProperties": false
            }),
        },
    ]
}

// ── Provider-specific converters ────────────────────────────

/// Convert tool definitions to Anthropic's `tools` array format.
///
/// Each entry becomes: `{name, description, input_schema: parameters}`.
#[must_use]
pub fn to_anthropic_tools(defs: &[ToolDefinition]) -> Vec<serde_json::Value> {
    defs.iter()
        .map(|d| {
            json!({
                "name": d.name,
                "description": d.description,
                "input_schema": d.parameters,
            })
        })
        .collect()
}

/// Convert tool definitions to `OpenAI`'s `tools` array format.
///
/// Each entry becomes: `{type: "function", function: {name, description, parameters}}`.
#[must_use]
pub fn to_openai_tools(defs: &[ToolDefinition]) -> Vec<serde_json::Value> {
    defs.iter()
        .map(|d| {
            json!({
                "type": "function",
                "function": {
                    "name": d.name,
                    "description": d.description,
                    "parameters": d.parameters,
                }
            })
        })
        .collect()
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn recursive_true_has_expected_tool_count() {
        let defs = tool_definitions(true);
        let expected = if cfg!(feature = "coraline") { 24 } else { 20 };
        assert_eq!(defs.len(), expected);
    }

    #[test]
    fn recursive_false_has_expected_tool_count() {
        let defs = tool_definitions(false);
        let expected = if cfg!(feature = "coraline") { 22 } else { 18 };
        assert_eq!(defs.len(), expected);
    }

    #[test]
    fn subtask_absent_when_not_recursive() {
        let defs = tool_definitions(false);
        assert!(!defs.iter().any(|d| d.name == "subtask"));
        assert!(!defs.iter().any(|d| d.name == "execute"));
    }

    #[test]
    fn subtask_present_when_recursive() {
        let defs = tool_definitions(true);
        assert!(defs.iter().any(|d| d.name == "subtask"));
        assert!(defs.iter().any(|d| d.name == "execute"));
    }

    #[test]
    fn all_tool_names_unique() {
        let defs = tool_definitions(true);
        let count = defs.len();
        let mut names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn to_openai_tools_has_type_function() {
        let defs = tool_definitions(true);
        let openai = to_openai_tools(&defs);
        for tool in &openai {
            assert_eq!(tool["type"], "function");
            assert!(tool["function"]["name"].is_string());
            assert!(tool["function"]["description"].is_string());
            assert!(tool["function"]["parameters"].is_object());
        }
    }

    #[test]
    fn to_anthropic_tools_has_input_schema() {
        let defs = tool_definitions(true);
        let anthropic = to_anthropic_tools(&defs);
        for tool in &anthropic {
            assert!(tool["name"].is_string());
            assert!(tool["description"].is_string());
            assert!(tool["input_schema"].is_object());
            // Should NOT have top-level "parameters" key
            assert!(tool.get("parameters").is_none());
        }
    }

    #[test]
    fn parallel_write_group_tools_present() {
        let defs = tool_definitions(false);
        assert!(defs.iter().any(|d| d.name == "begin_parallel_write_group"));
        assert!(defs.iter().any(|d| d.name == "end_parallel_write_group"));
    }

    #[test]
    fn parameters_are_json_schema_objects() {
        let defs = tool_definitions(true);
        for def in &defs {
            assert_eq!(
                def.parameters["type"], "object",
                "tool {} must have type=object",
                def.name
            );
            assert!(
                def.parameters.get("properties").is_some(),
                "tool {} must have properties",
                def.name
            );
        }
    }

    #[test]
    fn base_tools_include_core_filesystem() {
        let defs = tool_definitions(false);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"edit_file"));
        assert!(names.contains(&"apply_patch"));
    }

    #[test]
    fn base_tools_include_shell() {
        let defs = tool_definitions(false);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"run_shell"));
        assert!(names.contains(&"run_shell_bg"));
        assert!(names.contains(&"check_shell_bg"));
        assert!(names.contains(&"kill_shell_bg"));
        assert!(names.contains(&"cleanup_bg_jobs"));
    }

    #[test]
    fn base_tools_include_web() {
        let defs = tool_definitions(false);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"web_search"));
        assert!(names.contains(&"fetch_url"));
    }

    #[test]
    fn openai_converter_count_matches() {
        let defs = tool_definitions(true);
        let openai = to_openai_tools(&defs);
        assert_eq!(openai.len(), defs.len());
    }

    #[test]
    fn anthropic_converter_count_matches() {
        let defs = tool_definitions(true);
        let anthropic = to_anthropic_tools(&defs);
        assert_eq!(anthropic.len(), defs.len());
    }

    #[test]
    #[cfg(not(feature = "coraline"))]
    fn coraline_tools_absent_without_feature() {
        let defs = tool_definitions(true);
        assert!(!defs.iter().any(|d| d.name.starts_with("coraline_")));
    }

    #[test]
    #[cfg(feature = "coraline")]
    fn coraline_tools_present_with_feature() {
        let defs = tool_definitions(true);
        assert!(defs.iter().any(|d| d.name == "coraline_read_file"));
        assert!(defs.iter().any(|d| d.name == "coraline_search"));
        assert!(defs.iter().any(|d| d.name == "coraline_repo_map"));
        assert!(defs.iter().any(|d| d.name == "coraline_edit_file"));
    }
}
