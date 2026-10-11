//! MCP tool registry
//!
//! Every tool delegates to the same functions the in-app agent uses, so the
//! path policy, timeouts and output formats are identical for both.

use serde_json::{json, Map, Value};

use crate::commands::agent::commands::{execute_agent_command, run_agent_instrument};
use crate::commands::agent::files::{
    agent_copy_file, agent_edit_file, agent_find_exe, agent_glob, agent_grep, agent_list_dir,
    agent_move_file, agent_read_file, agent_write_file, list_agent_programs, list_instruments,
};
use crate::commands::agent::search::search_web;
use crate::commands::system_info::get_system_info;

/// One MCP tool: its listing metadata and JSON input schema.
struct ToolSpec {
    name: &'static str,
    description: &'static str,
    properties: fn() -> Value,
    required: &'static [&'static str],
}

fn no_props() -> Value {
    json!({})
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "execute_command",
        description: "Execute a PowerShell command on the system. Returns stdout, stderr, and exit code.",
        properties: || json!({
            "command": { "type": "string", "description": "The PowerShell command to execute" },
            "reason": { "type": "string", "description": "Brief explanation of why this command is needed" }
        }),
        required: &["command", "reason"],
    },
    ToolSpec {
        name: "read_file",
        description: "Read a text file with optional pagination and line numbers.",
        properties: || json!({
            "path": { "type": "string", "description": "Full path to the file" },
            "offset": { "type": "integer", "description": "Line to start from (0-indexed)" },
            "limit": { "type": "integer", "description": "Maximum number of lines" },
            "line_numbers": { "type": "boolean", "description": "Prefix lines with numbers (default true)" }
        }),
        required: &["path"],
    },
    ToolSpec {
        name: "write_file",
        description: "Write content to a file. Creates the file (and parent folders) if needed.",
        properties: || json!({
            "path": { "type": "string", "description": "Full path to the file" },
            "content": { "type": "string", "description": "Content to write" }
        }),
        required: &["path", "content"],
    },
    ToolSpec {
        name: "edit_file",
        description: "Replace old_string with new_string in a file. old_string must be unique unless all=true.",
        properties: || json!({
            "path": { "type": "string", "description": "Full path to the file" },
            "old_string": { "type": "string", "description": "Exact text to replace" },
            "new_string": { "type": "string", "description": "Replacement text (may be empty)" },
            "all": { "type": "boolean", "description": "Replace every occurrence" }
        }),
        required: &["path", "old_string", "new_string"],
    },
    ToolSpec {
        name: "list_dir",
        description: "List files and directories in a path.",
        properties: || json!({
            "path": { "type": "string", "description": "Directory to list" }
        }),
        required: &["path"],
    },
    ToolSpec {
        name: "move_file",
        description: "Move or rename a file.",
        properties: || json!({
            "src": { "type": "string", "description": "Source path" },
            "dest": { "type": "string", "description": "Destination path" }
        }),
        required: &["src", "dest"],
    },
    ToolSpec {
        name: "copy_file",
        description: "Copy a file.",
        properties: || json!({
            "src": { "type": "string", "description": "Source path" },
            "dest": { "type": "string", "description": "Destination path" }
        }),
        required: &["src", "dest"],
    },
    ToolSpec {
        name: "grep",
        description: "Search for a regex pattern across files under a directory. Returns file, line and content.",
        properties: || json!({
            "pattern": { "type": "string", "description": "Regex pattern" },
            "path": { "type": "string", "description": "Directory to search (default: working directory)" },
            "file_pattern": { "type": "string", "description": "Glob filter for file names, e.g. *.log" },
            "max_results": { "type": "integer", "description": "Maximum matches (default 50)" }
        }),
        required: &["pattern"],
    },
    ToolSpec {
        name: "glob",
        description: "Find files matching a glob pattern, newest first.",
        properties: || json!({
            "pattern": { "type": "string", "description": "Glob pattern relative to path, e.g. **/*.log" },
            "path": { "type": "string", "description": "Base directory (default: working directory)" },
            "limit": { "type": "integer", "description": "Maximum results (default 100)" }
        }),
        required: &["pattern"],
    },
    ToolSpec {
        name: "get_system_info",
        description: "Get system information: OS, CPU, memory, disks, GPU, network.",
        properties: no_props,
        required: &[],
    },
    ToolSpec {
        name: "search_web",
        description: "Search the web with the provider configured in RustService settings.",
        properties: || json!({
            "query": { "type": "string", "description": "The search query" }
        }),
        required: &["query"],
    },
    ToolSpec {
        name: "list_programs",
        description: "List the portable programs in the data/programs folder and their executables.",
        properties: no_props,
        required: &[],
    },
    ToolSpec {
        name: "find_exe",
        description: "Find executables by name in data/programs (and optionally the system PATH).",
        properties: || json!({
            "query": { "type": "string", "description": "Executable name or keyword" },
            "search_path": { "type": "boolean", "description": "Also search the system PATH" }
        }),
        required: &["query"],
    },
    ToolSpec {
        name: "list_instruments",
        description: "List the custom instruments (scripts) that can be run.",
        properties: no_props,
        required: &[],
    },
    ToolSpec {
        name: "run_instrument",
        description: "Run a custom instrument (script) by name.",
        properties: || json!({
            "name": { "type": "string", "description": "Instrument name" },
            "args": { "type": "string", "description": "Optional arguments" }
        }),
        required: &["name"],
    },
];

/// Number of tools exposed (for the health endpoint).
pub fn tool_count() -> usize {
    TOOLS.len()
}

/// The `tools/list` result.
pub fn definitions() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": {
                    "type": "object",
                    "properties": (t.properties)(),
                    "required": t.required,
                    "additionalProperties": false
                }
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// Whether `name` is a known tool.
pub fn exists(name: &str) -> bool {
    TOOLS.iter().any(|t| t.name == name)
}

fn str_arg(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("Missing required argument: {}", key))
}

fn opt_str(args: &Map<String, Value>, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn opt_usize(args: &Map<String, Value>, key: &str) -> Option<usize> {
    args.get(key)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
}

fn opt_bool(args: &Map<String, Value>, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

fn pretty(value: impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|e| format!("Failed to encode result: {}", e))
}

/// Run blocking tool work off the async runtime.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("Tool task failed: {}", e))?
}

fn format_command_result(cmd: &crate::types::PendingCommand) -> String {
    let mut out = format!(
        "Status: {}\n\nOutput:\n{}",
        if cmd.status == crate::types::CommandStatus::Executed {
            "success"
        } else {
            "failed"
        },
        cmd.output.as_deref().unwrap_or("")
    );
    if let Some(err) = cmd.error.as_deref().filter(|e| !e.is_empty()) {
        out.push_str("\n\nErrors:\n");
        out.push_str(err);
    }
    out
}

/// Execute a tool and return its text output. `Err` means the tool itself
/// failed (reported to the client as `isError: true`).
pub async fn call(name: &str, args: Map<String, Value>) -> Result<String, String> {
    match name {
        "execute_command" => {
            let command = str_arg(&args, "command")?;
            let reason = opt_str(&args, "reason").unwrap_or_else(|| "MCP request".to_string());
            log::info!("[MCP] execute_command: {} ({})", command, reason);
            let result = execute_agent_command(command, format!("MCP: {}", reason)).await?;
            Ok(format_command_result(&result))
        }
        "read_file" => {
            let path = str_arg(&args, "path")?;
            let (offset, limit, numbers) = (
                opt_usize(&args, "offset"),
                opt_usize(&args, "limit"),
                opt_bool(&args, "line_numbers"),
            );
            let result = blocking(move || agent_read_file(path, offset, limit, numbers)).await?;
            Ok(format!(
                "{}\n\n[total lines: {}, more available: {}]",
                result.content, result.total_lines, result.has_more
            ))
        }
        "write_file" => {
            let path = str_arg(&args, "path")?;
            let content = str_arg(&args, "content")?;
            let shown = path.clone();
            blocking(move || agent_write_file(path, content)).await?;
            Ok(format!("Wrote {}", shown))
        }
        "edit_file" => {
            let path = str_arg(&args, "path")?;
            let old = str_arg(&args, "old_string")?;
            let new = str_arg(&args, "new_string")?;
            let all = opt_bool(&args, "all");
            let result = blocking(move || agent_edit_file(path, old, new, all)).await?;
            if result.status == "success" {
                Ok(result.message)
            } else {
                Err(result.message)
            }
        }
        "list_dir" => {
            let path = str_arg(&args, "path")?;
            pretty(blocking(move || agent_list_dir(path)).await?)
        }
        "move_file" => {
            let (src, dest) = (str_arg(&args, "src")?, str_arg(&args, "dest")?);
            let msg = format!("Moved {} to {}", src, dest);
            blocking(move || agent_move_file(src, dest)).await?;
            Ok(msg)
        }
        "copy_file" => {
            let (src, dest) = (str_arg(&args, "src")?, str_arg(&args, "dest")?);
            let msg = format!("Copied {} to {}", src, dest);
            blocking(move || agent_copy_file(src, dest)).await?;
            Ok(msg)
        }
        "grep" => {
            let pattern = str_arg(&args, "pattern")?;
            let (path, file_pattern, max) = (
                opt_str(&args, "path"),
                opt_str(&args, "file_pattern"),
                opt_usize(&args, "max_results"),
            );
            pretty(blocking(move || agent_grep(pattern, path, file_pattern, max)).await?)
        }
        "glob" => {
            let pattern = str_arg(&args, "pattern")?;
            let (path, limit) = (opt_str(&args, "path"), opt_usize(&args, "limit"));
            pretty(blocking(move || agent_glob(pattern, path, limit)).await?)
        }
        "get_system_info" => pretty(get_system_info().await?),
        "search_web" => {
            let query = str_arg(&args, "query")?;
            pretty(search_web(&query).await?)
        }
        "list_programs" => pretty(blocking(list_agent_programs).await?),
        "find_exe" => {
            let query = str_arg(&args, "query")?;
            let search_path = opt_bool(&args, "search_path");
            pretty(blocking(move || agent_find_exe(query, search_path)).await?)
        }
        "list_instruments" => pretty(blocking(list_instruments).await?),
        "run_instrument" => {
            let name = str_arg(&args, "name")?;
            let instrument_args = opt_str(&args, "args");
            let result = run_agent_instrument(name, instrument_args).await?;
            Ok(format_command_result(&result))
        }
        other => Err(format!("Unknown tool: {}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_tool_is_dispatched() {
        // A tool in TOOLS without a match arm would only surface at runtime.
        let src = include_str!("tools.rs");
        for t in TOOLS {
            assert!(
                src.contains(&format!("\"{}\" =>", t.name)),
                "no handler for {}",
                t.name
            );
        }
        let defs = definitions();
        assert_eq!(defs["tools"].as_array().unwrap().len(), TOOLS.len());
    }
}
