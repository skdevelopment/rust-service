//! CHKDSK Filesystem Check Service
//!
//! Runs Windows built-in `chkdsk.exe` to check filesystem integrity.
//! Supports three modes: read_only, fix_errors (/f), and comprehensive (/f /r).

use regex::Regex;
use serde_json::json;

use crate::services::{Service, ServiceCtx};
use crate::types::{
    FindingSeverity, OptionType, SelectOption, ServiceDefinition, ServiceFinding,
    ServiceOptionSchema, ServiceResult,
};

// =============================================================================
// Service Implementation
// =============================================================================

pub struct ChkdskService;

impl Service for ChkdskService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "chkdsk".to_string(),
            name: "Disk Check (CHKDSK)".to_string(),
            description: "Check filesystem integrity and optionally repair errors".to_string(),
            category: "maintenance".to_string(),
            estimated_duration_secs: 300,
            required_programs: vec![], // Built-in Windows tool
            options: vec![
                ServiceOptionSchema {
                    id: "drive".to_string(),
                    label: "Drive".to_string(),
                    option_type: OptionType::String,
                    default_value: json!("C:"),
                    min: None,
                    max: None,
                    options: None,
                    description: Some("Drive letter to check (e.g., C:, D:)".to_string()),
                },
                ServiceOptionSchema {
                    id: "mode".to_string(),
                    label: "Check Mode".to_string(),
                    option_type: OptionType::Select,
                    default_value: json!("read_only"),
                    min: None,
                    max: None,
                    options: Some(vec![
                        SelectOption {
                            value: "read_only".to_string(),
                            label: "Read Only (Safe)".to_string(),
                        },
                        SelectOption {
                            value: "fix_errors".to_string(),
                            label: "Fix Errors (/f)".to_string(),
                        },
                        SelectOption {
                            value: "comprehensive".to_string(),
                            label: "Comprehensive (/f /r)".to_string(),
                        },
                    ]),
                    description: Some(
                        "Scan mode - read_only is safe, fix_errors may require reboot".to_string(),
                    ),
                },
                ServiceOptionSchema {
                    id: "schedule_if_busy".to_string(),
                    label: "Schedule if Volume Busy".to_string(),
                    option_type: OptionType::Boolean,
                    default_value: json!(false),
                    min: None,
                    max: None,
                    options: None,
                    description: Some(
                        "Automatically schedule check for next reboot if volume is in use"
                            .to_string(),
                    ),
                },
            ],
            icon: "hard-drive-download".to_string(),
            exclusive_resources: vec!["disk-exclusive".to_string()],
            dependencies: vec![],
        }
    }

    fn run(&self, options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        // Parse options
        let drive = normalize_drive(
            options
                .get("drive")
                .and_then(|v| v.as_str())
                .unwrap_or("C:"),
        );
        let mode = options
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("read_only");
        let schedule_if_busy = options
            .get("schedule_if_busy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        ctx.log(format!("Starting CHKDSK on {} (mode: {})", drive, mode));

        // Validate mode
        if !["read_only", "fix_errors", "comprehensive"].contains(&mode) {
            ctx.log(format!("ERROR: Invalid mode: {}", mode));
            ctx.findings.push(ServiceFinding {
                severity: FindingSeverity::Error,
                title: "Invalid Mode".to_string(),
                description: format!(
                    "Mode '{}' is not valid. Use read_only, fix_errors, or comprehensive.",
                    mode
                ),
                recommendation: None,
                data: Some(json!({"type": "error", "reason": "invalid_mode"})),
            });
            return ctx.finish(false, Some(format!("Invalid mode: {}", mode)));
        }

        // Build command arguments
        let mut args = vec![drive.clone()];
        if mode == "fix_errors" {
            args.push("/f".to_string());
        } else if mode == "comprehensive" {
            args.push("/f".to_string());
            args.push("/r".to_string());
        }

        ctx.log(format!("Running: chkdsk {}", args.join(" ")));

        if mode != "read_only" {
            ctx.log("Note: Fix modes may require exclusive access or reboot");
        }

        // Execute CHKDSK
        let mut cmd = crate::proc::command("chkdsk");
        cmd.args(&args);

        // If the volume is busy, chkdsk asks whether to schedule the check for
        // the next restart; answer "Y" when the user opted into that.
        let output = if schedule_if_busy && mode != "read_only" {
            ctx.output_with_input(&mut cmd, b"Y\r\n")
        } else {
            ctx.output(&mut cmd)
        };

        let output = match output {
            Ok(output) => output,
            Err(e) => {
                ctx.log(format!("ERROR: Failed to run CHKDSK: {}", e));
                ctx.findings.push(ServiceFinding {
                    severity: FindingSeverity::Error,
                    title: "Execution Failed".to_string(),
                    description: format!("Could not execute CHKDSK: {}", e),
                    recommendation: Some(
                        "Ensure you are running with administrator privileges.".to_string(),
                    ),
                    data: Some(json!({"type": "error", "reason": "execution_failed"})),
                });
                return ctx.finish(false, Some(format!("CHKDSK execution failed: {}", e)));
            }
        };

        let stdout = crate::proc::decode_output(&output.stdout).to_string();
        let stderr = crate::proc::decode_output(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        ctx.log(format!("CHKDSK completed with exit code: {}", exit_code));

        // Parse output
        let parsed = parse_chkdsk_output(&stdout);

        // Log key findings
        if parsed.found_no_problems {
            ctx.log("No problems found on disk");
        }
        if parsed.errors_found {
            ctx.log("Errors were detected on disk");
        }
        if parsed.made_corrections {
            ctx.log("Corrections were made to the filesystem");
        }
        if parsed.volume_in_use {
            ctx.log("Volume is currently in use");
        }
        if let Some(ref fs_type) = parsed.filesystem_type {
            ctx.log(format!("Filesystem type: {}", fs_type));
        }

        // Determine status and create findings
        let (severity, title, description) = if parsed.access_denied {
            (
                FindingSeverity::Error,
                "Access Denied".to_string(),
                "CHKDSK requires administrator privileges.".to_string(),
            )
        } else if parsed.invalid_drive {
            (
                FindingSeverity::Error,
                "Invalid Drive".to_string(),
                format!("Drive {} not found or is invalid.", drive),
            )
        } else if parsed.found_no_problems {
            (
                FindingSeverity::Success,
                "Disk Healthy".to_string(),
                format!("No problems found on {}. Filesystem is healthy.", drive),
            )
        } else if parsed.made_corrections {
            (
                FindingSeverity::Success,
                "Issues Repaired".to_string(),
                format!("Filesystem errors on {} were successfully repaired.", drive),
            )
        } else if parsed.errors_found && mode == "read_only" {
            (
                FindingSeverity::Warning,
                "Errors Detected".to_string(),
                format!(
                    "Errors found on {}. Run with fix_errors mode to repair.",
                    drive
                ),
            )
        } else if parsed.volume_in_use {
            if schedule_if_busy {
                (
                    FindingSeverity::Info,
                    "Scheduled for Reboot".to_string(),
                    format!("Volume {} is in use. Check scheduled for next boot.", drive),
                )
            } else {
                (
                    FindingSeverity::Warning,
                    "Volume In Use".to_string(),
                    format!(
                        "Cannot check {} while in use. Enable 'Schedule if Busy' option.",
                        drive
                    ),
                )
            }
        } else if exit_code == 0 {
            (
                FindingSeverity::Success,
                "Scan Complete".to_string(),
                format!("CHKDSK completed successfully on {}.", drive),
            )
        } else {
            (
                FindingSeverity::Warning,
                "Check Complete".to_string(),
                format!("CHKDSK finished with exit code {}.", exit_code),
            )
        };

        ctx.findings.push(ServiceFinding {
            severity,
            title,
            description,
            recommendation: if parsed.errors_found && mode == "read_only" {
                Some("Run CHKDSK with 'Fix Errors' mode to repair detected issues.".to_string())
            } else if parsed.volume_in_use && !schedule_if_busy {
                Some("Enable 'Schedule if Busy' to check this volume on next reboot.".to_string())
            } else {
                None
            },
            data: Some(json!({
                "type": "chkdsk_result",
                "drive": drive,
                "mode": mode,
                "filesystemType": parsed.filesystem_type,
                "foundNoProblems": parsed.found_no_problems,
                "errorsFound": parsed.errors_found,
                "madeCorrections": parsed.made_corrections,
                "volumeInUse": parsed.volume_in_use,
                "accessDenied": parsed.access_denied,
                "invalidDrive": parsed.invalid_drive,
                "totalDiskKb": parsed.total_disk_kb,
                "availableKb": parsed.available_kb,
                "badSectorsKb": parsed.bad_sectors_kb,
                "inFilesKb": parsed.in_files_kb,
                "systemUseKb": parsed.system_use_kb,
                "durationSeconds": parsed.duration_seconds,
                "exitCode": exit_code,
            })),
        });

        // Log stderr if present
        if !stderr.is_empty() && stderr.len() < 500 {
            ctx.log(format!("Stderr: {}", stderr.trim()));
        }

        ctx.log("CHKDSK scan complete");

        ctx.finish(
            exit_code == 0 || parsed.found_no_problems || parsed.made_corrections,
            None,
        )
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Normalize drive letter to standard format (e.g., "C:")
fn normalize_drive(drive: &str) -> String {
    let d = drive.trim().replace("/", "\\");

    // Handle various formats: "C", "C:", "C:\\", "C:\\path"
    if d.is_empty() {
        return "C:".to_string();
    }

    let first_char = d.chars().next().unwrap().to_ascii_uppercase();
    if !first_char.is_ascii_alphabetic() {
        return "C:".to_string();
    }

    format!("{}:", first_char)
}

// =============================================================================
// Output Parsing
// =============================================================================

#[derive(Debug, Default)]
struct ChkdskParsedOutput {
    found_no_problems: bool,
    errors_found: bool,
    volume_in_use: bool,
    made_corrections: bool,
    access_denied: bool,
    invalid_drive: bool,
    filesystem_type: Option<String>,
    total_disk_kb: Option<u64>,
    in_files_kb: Option<u64>,
    bad_sectors_kb: Option<u64>,
    system_use_kb: Option<u64>,
    available_kb: Option<u64>,
    duration_seconds: Option<f64>,
}

fn parse_chkdsk_output(output: &str) -> ChkdskParsedOutput {
    let mut result = ChkdskParsedOutput::default();

    // Boolean pattern checks
    let lower = output.to_lowercase();

    result.found_no_problems =
        lower.contains("found no problems") || lower.contains("no further action is required");

    result.errors_found = lower.contains("windows found errors")
        || lower.contains("errors found")
        || lower.contains("chkdsk cannot continue");

    result.volume_in_use = lower.contains("volume is in use")
        || lower.contains("cannot lock")
        || lower.contains("cannot run because");

    result.made_corrections = lower.contains("made corrections to the file system");

    result.access_denied =
        lower.contains("access denied") || lower.contains("insufficient privileges");

    result.invalid_drive =
        lower.contains("cannot find the drive") || lower.contains("invalid drive");

    // Extract filesystem type
    if let Ok(re) = Regex::new(r"(?i)The type of the file system is (\w+)") {
        if let Some(caps) = re.captures(output) {
            result.filesystem_type = caps.get(1).map(|m| m.as_str().to_string());
        }
    }

    // Extract numeric values
    result.total_disk_kb = extract_kb_value(r"(\d[\d,]*)\s+KB total disk space", output);
    result.in_files_kb = extract_kb_value(r"(\d[\d,]*)\s+KB in \d+ files", output);
    result.bad_sectors_kb = extract_kb_value(r"(\d[\d,]*)\s+KB in bad sectors", output);
    result.system_use_kb = extract_kb_value(r"(\d[\d,]*)\s+KB in use by the system", output);
    result.available_kb = extract_kb_value(r"(\d[\d,]*)\s+KB available on disk", output);

    // Extract duration
    if let Ok(re) = Regex::new(r"Total duration:\s*[^()]*\((\d+)\s*ms\)") {
        if let Some(caps) = re.captures(output) {
            if let Ok(ms) = caps
                .get(1)
                .map_or("0", |m| m.as_str())
                .replace(",", "")
                .parse::<u64>()
            {
                result.duration_seconds = Some(ms as f64 / 1000.0);
            }
        }
    }

    result
}

fn extract_kb_value(pattern: &str, text: &str) -> Option<u64> {
    Regex::new(pattern).ok().and_then(|re| {
        re.captures(text).and_then(|caps| {
            caps.get(1)
                .map(|m| m.as_str().replace(",", ""))
                .and_then(|s| s.parse().ok())
        })
    })
}
