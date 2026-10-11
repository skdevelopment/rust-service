//! Windows PowerShell helper for the upgrade commands.
//!
//! Scripts are passed as UTF-16 encoded commands so paths keep their characters
//! and nothing is concatenated into a live shell string by the caller.

use std::time::Duration;

use base64::Engine;

use crate::proc;

/// Run `script` and return stdout. A non-zero exit is the stderr text, or
/// stdout when PowerShell put the message there instead.
pub(super) fn run(script: &str, timeout: Duration) -> Result<String, String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let output = proc::output_with_timeout(
        proc::command("powershell.exe").args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ]),
        timeout,
    )
    .map_err(|e| format!("PowerShell could not be started: {e}"))?;

    let stdout = proc::decode_output(&output.stdout);
    let stderr = proc::decode_output(&output.stderr);
    if output.status.success() {
        return Ok(stdout);
    }
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if detail.is_empty() {
        Err(format!("PowerShell exited with status {}.", output.status))
    } else {
        Err(detail.to_string())
    }
}

/// The outermost JSON object in `text`, so a PowerShell banner cannot hide it.
pub(super) fn json_object(text: &str) -> &str {
    let Some(start) = text.find('{') else {
        return text;
    };
    let Some(end) = text.rfind('}') else {
        return text;
    };
    if start < end {
        &text[start..=end]
    } else {
        text
    }
}

/// Quote `path` for a PowerShell single-quoted string.
pub(super) fn quote(path: &str) -> Result<String, String> {
    if path.chars().any(|c| c == '\n' || c == '\r' || c == '\0') {
        return Err("The path contains a newline.".into());
    }
    Ok(format!("'{}'", path.replace('\'', "''")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_escapes_single_quotes_only() {
        assert_eq!(quote(r"D:\ISOs\Win11.iso").unwrap(), r"'D:\ISOs\Win11.iso'");
        assert_eq!(quote(r"D:\It's.iso").unwrap(), r"'D:\It''s.iso'");
    }

    #[test]
    fn quote_rejects_newlines() {
        assert!(quote("D:\\\nWin.iso").is_err());
    }

    #[test]
    fn json_object_ignores_leading_noise() {
        assert_eq!(json_object("banner\r\n{\"ok\":true}\r\n"), "{\"ok\":true}");
    }
}
