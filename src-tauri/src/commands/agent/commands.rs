//! Command execution
//!
//! Approval happens in the chat UI (human-in-the-loop tool calls). The backend
//! only executes commands the UI has approved, or that the approval mode allows
//! automatically, and keeps an audit trail in the agent database.

use std::time::Duration;

use chrono::Utc;
use regex::Regex;
use rusqlite::params;
use uuid::Uuid;

use super::{
    get_db_connection, get_settings, CommandExecutionResult, CommandStatus, Instrument,
    PendingCommand,
};

/// Agent commands may legitimately run for a long time (DISM, SFC, scans),
/// but must not hang the agent forever.
const AGENT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Characters that let one "whitelisted" command smuggle in another
/// (`ipconfig; Remove-Item ...`, `ping x | ...`, `$(...)`, redirection).
const SHELL_METACHARS: &[char] = &[
    ';', '|', '&', '>', '<', '`', '\n', '\r', '$', '(', ')', '{', '}',
];

/// Whether `command` may run without approval in whitelist mode.
///
/// The command must be a single simple command (no chaining, pipes,
/// redirection, subexpressions or variables) and match one of the patterns.
pub(crate) fn is_command_whitelisted(command: &str, patterns: &[String]) -> bool {
    let command = command.trim();
    if command.is_empty() || command.contains(SHELL_METACHARS) {
        return false;
    }
    patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .any(|re| re.is_match(command))
}

/// Run a PowerShell command and capture its output.
///
/// Shared by the agent tools and the MCP server.
pub(crate) fn run_shell_command(command: &str) -> Result<CommandExecutionResult, String> {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = crate::proc::command("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-Command", command]);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = crate::proc::command("sh");
        c.args(["-c", command]);
        c
    };

    let output = crate::proc::output_with_timeout(&mut cmd, AGENT_COMMAND_TIMEOUT)
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    Ok(CommandExecutionResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: crate::proc::decode_output(&output.stdout),
        stderr: crate::proc::decode_output(&output.stderr),
    })
}

/// Run a command and record it in the command history.
fn execute_and_log(command: String, reason: String) -> Result<PendingCommand, String> {
    let result = run_shell_command(&command)?;

    let executed = PendingCommand {
        id: Uuid::new_v4().to_string(),
        command,
        reason,
        created_at: Utc::now().to_rfc3339(),
        status: if result.exit_code == 0 {
            CommandStatus::Executed
        } else {
            CommandStatus::Failed
        },
        output: Some(result.stdout),
        error: if result.stderr.is_empty() {
            None
        } else {
            Some(result.stderr)
        },
    };

    if let Err(e) = log_command_to_history(&executed) {
        log::warn!("Failed to log agent command: {}", e);
    }

    Ok(executed)
}

/// Execute a command the user approved in the chat (or that the approval mode
/// allows automatically).
#[tauri::command]
#[specta::specta]
pub async fn execute_agent_command(
    command: String,
    reason: String,
) -> Result<PendingCommand, String> {
    tauri::async_runtime::spawn_blocking(move || execute_and_log(command, reason))
        .await
        .map_err(|e| format!("Command task failed: {}", e))?
}

/// Whether the given command may run without approval in whitelist mode
#[tauri::command(async)]
#[specta::specta]
pub fn check_command_whitelisted(command: String) -> Result<bool, String> {
    let settings = get_settings()?;
    Ok(is_command_whitelisted(
        &command,
        &settings.agent.whitelisted_commands,
    ))
}

/// Build the shell command that runs an instrument with optional arguments.
pub(crate) fn instrument_command(
    instrument: &Instrument,
    args: Option<&str>,
) -> Result<String, String> {
    let path = instrument.path.replace('\'', "''");
    let args = args.unwrap_or("").trim();
    let command = match instrument.extension.to_lowercase().as_str() {
        "ps1" => format!("& '{}' {}", path, args),
        "bat" | "cmd" | "exe" => format!("& '{}' {}", path, args),
        "py" => format!("& python '{}' {}", path, args),
        "js" => format!("& node '{}' {}", path, args),
        other => return Err(format!("Unsupported instrument type: .{}", other)),
    };
    Ok(command.trim_end().to_string())
}

/// Find an instrument by (case-insensitive) name.
pub(crate) fn find_instrument(name: &str) -> Result<Instrument, String> {
    super::files::list_instruments()?
        .into_iter()
        .find(|i| i.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            format!(
                "Instrument '{}' not found. Use list_instruments to see available ones.",
                name
            )
        })
}

/// Run an instrument the user approved in the chat
#[tauri::command]
#[specta::specta]
pub async fn run_agent_instrument(
    name: String,
    args: Option<String>,
) -> Result<PendingCommand, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let instrument = find_instrument(&name)?;
        let command = instrument_command(&instrument, args.as_deref())?;
        execute_and_log(command, format!("Running instrument: {}", instrument.name))
    })
    .await
    .map_err(|e| format!("Instrument task failed: {}", e))?
}

fn log_command_to_history(cmd: &PendingCommand) -> Result<(), String> {
    let conn = get_db_connection()?;

    let status_str = match cmd.status {
        CommandStatus::Pending => "pending",
        CommandStatus::Approved => "approved",
        CommandStatus::Rejected => "rejected",
        CommandStatus::Executed => "executed",
        CommandStatus::Failed => "failed",
    };

    conn.execute(
        "INSERT INTO command_history (id, command, reason, status, output, error, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            cmd.id,
            cmd.command,
            cmd.reason,
            status_str,
            cmd.output,
            cmd.error,
            cmd.created_at
        ],
    )
    .map_err(|e| format!("Failed to log command: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns() -> Vec<String> {
        vec!["^ipconfig".into(), "^ping ".into(), "^hostname$".into()]
    }

    #[test]
    fn whitelist_matches_simple_commands() {
        assert!(is_command_whitelisted("ipconfig /all", &patterns()));
        assert!(is_command_whitelisted("ping 8.8.8.8", &patterns()));
        assert!(is_command_whitelisted("  hostname ", &patterns()));
        assert!(!is_command_whitelisted("hostname2", &patterns()));
    }

    #[test]
    fn whitelist_rejects_chaining_and_subexpressions() {
        for cmd in [
            "ipconfig; Remove-Item C:\\x",
            "ipconfig | Out-File x",
            "ping 1.1.1.1 && del x",
            "ping $(Remove-Item x)",
            "ipconfig > out.txt",
            "ping `\nRemove-Item x",
            "ping $env:COMPUTERNAME",
        ] {
            assert!(!is_command_whitelisted(cmd, &patterns()), "{cmd}");
        }
    }

    #[test]
    fn instrument_paths_are_quoted() {
        let i = Instrument {
            name: "x".into(),
            description: String::new(),
            path: "C:\\My Tools\\it's.ps1".into(),
            extension: "ps1".into(),
        };
        assert_eq!(
            instrument_command(&i, Some("-Verbose")).unwrap(),
            "& 'C:\\My Tools\\it''s.ps1' -Verbose"
        );
    }
}
