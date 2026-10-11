//! Service report storage, editing, statistics and text export

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use uuid::Uuid;

use super::runner;
use crate::commands::data_dir::get_data_dir_path;
use crate::commands::settings::get_settings;
use crate::services;
use crate::types::{
    FindingSeverity, FindingSeverityCounts, ReportStatistics, ServiceDefinition, ServiceFinding,
    ServiceReport, ServiceResult,
};

// =============================================================================
// Health Score Constants
// =============================================================================

const CRITICAL_FINDING_PENALTY: i32 = 30;
const ERROR_FINDING_PENALTY: i32 = 15;
const WARNING_FINDING_PENALTY: i32 = 5;
const SUCCESS_FINDING_BONUS: i32 = 2;
const HEALTH_SCORE_BASE: i32 = 100;

/// Milliseconds per second (for duration formatting)
const MS_PER_SECOND: f64 = 1000.0;

// =============================================================================
// Storage
// =============================================================================

fn get_reports_dir() -> PathBuf {
    get_data_dir_path().join("reports")
}

/// Report ids are UUIDs we generate; reject anything else before touching disk.
fn report_path(report_id: &str) -> Result<PathBuf, String> {
    Uuid::parse_str(report_id).map_err(|_| format!("Invalid report id: {}", report_id))?;
    Ok(get_reports_dir().join(format!("{}.json", report_id)))
}

fn save_report(report: &ServiceReport) -> Result<(), String> {
    let path = report_path(&report.id)?;
    fs::create_dir_all(get_reports_dir())
        .map_err(|e| format!("Failed to create reports dir: {}", e))?;
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| format!("Failed to serialize report: {}", e))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("Failed to write report: {}", e))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Failed to write report: {}", e))?;
    runner::update_current_report(report);
    Ok(())
}

/// Save a just-finished run according to the report settings, then apply retention.
pub(super) fn persist_completed_report(report: &ServiceReport) -> Result<(), String> {
    let settings = get_settings().unwrap_or_default().reports;
    if !settings.auto_save_reports {
        // Kept in memory (and editable) until the next run or an explicit save.
        return Ok(());
    }
    let mut to_save = report.clone();
    if !settings.include_logs_in_report {
        for result in &mut to_save.results {
            result.logs.clear();
        }
    }
    save_report(&to_save)?;
    if let Err(e) = apply_retention(settings.report_retention_days) {
        log::warn!("Report retention cleanup failed: {}", e);
    }
    Ok(())
}

/// Delete saved reports older than `days` (0 keeps everything).
pub(crate) fn apply_retention(days: u32) -> Result<u32, String> {
    if days == 0 {
        return Ok(0);
    }
    let cutoff = Utc::now() - chrono::Duration::days(i64::from(days));
    let mut deleted = 0;
    for report in list_service_reports()? {
        let started = chrono::DateTime::parse_from_rfc3339(&report.started_at);
        if started.is_ok_and(|t| t < cutoff) && fs::remove_file(report_path(&report.id)?).is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Load a report from disk, falling back to the in-memory current report
/// (which is not on disk when auto-save is disabled).
fn load_report(report_id: &str) -> Result<ServiceReport, String> {
    let path = report_path(report_id)?;
    match fs::read_to_string(&path) {
        Ok(json) => {
            serde_json::from_str(&json).map_err(|e| format!("Failed to parse report: {}", e))
        }
        Err(e) => runner::current_report()
            .filter(|r| r.id == report_id)
            .ok_or_else(|| format!("Failed to read report: {}", e)),
    }
}

/// Get a saved report by ID
#[tauri::command(async)]
#[specta::specta]
pub fn get_service_report(report_id: String) -> Result<ServiceReport, String> {
    load_report(&report_id)
}

/// List all saved reports
#[tauri::command(async)]
#[specta::specta]
pub fn list_service_reports() -> Result<Vec<ServiceReport>, String> {
    let reports_dir = get_reports_dir();
    if !reports_dir.exists() {
        return Ok(Vec::new());
    }

    let entries =
        fs::read_dir(&reports_dir).map_err(|e| format!("Failed to read reports dir: {}", e))?;
    let mut reports: Vec<ServiceReport> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|json| serde_json::from_str(&json).ok())
        .collect();

    reports.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(reports)
}

/// Delete a saved report by ID
#[tauri::command(async)]
#[specta::specta]
pub fn delete_report(report_id: String) -> Result<(), String> {
    let path = report_path(&report_id)?;
    if !path.exists() {
        return Err(format!("Report not found: {}", report_id));
    }
    fs::remove_file(&path).map_err(|e| format!("Failed to delete report: {}", e))
}

/// Delete all saved reports
#[tauri::command(async)]
#[specta::specta]
pub fn clear_all_reports() -> Result<u32, String> {
    let reports_dir = get_reports_dir();
    if !reports_dir.exists() {
        return Ok(0);
    }

    let entries =
        fs::read_dir(&reports_dir).map_err(|e| format!("Failed to read reports dir: {}", e))?;
    let deleted = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter(|e| fs::remove_file(e.path()).is_ok())
        .count();
    Ok(deleted as u32)
}

// =============================================================================
// Editing (agent tools)
// =============================================================================

fn parse_severity(severity: &str) -> Result<FindingSeverity, String> {
    match severity {
        "info" => Ok(FindingSeverity::Info),
        "success" => Ok(FindingSeverity::Success),
        "warning" => Ok(FindingSeverity::Warning),
        "error" => Ok(FindingSeverity::Error),
        "critical" => Ok(FindingSeverity::Critical),
        _ => Err(format!("Invalid severity: {}", severity)),
    }
}

/// Load, modify and save one service result of a report.
///
/// `queue_item_id` disambiguates when the same service ran more than once.
fn edit_result(
    report_id: &str,
    service_id: &str,
    queue_item_id: Option<&str>,
    edit: impl FnOnce(&mut ServiceResult) -> Result<(), String>,
) -> Result<(), String> {
    let mut report = load_report(report_id)?;
    let result = report
        .results
        .iter_mut()
        .find(|r| {
            r.service_id == service_id
                && queue_item_id.is_none_or(|id| r.queue_item_id.as_deref() == Some(id))
        })
        .ok_or_else(|| format!("Service result not found: {}", service_id))?;
    edit(result)?;
    save_report(&report)
}

/// Edit an existing finding in a report
#[tauri::command(async)]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub fn edit_report_finding(
    report_id: String,
    service_id: String,
    finding_index: usize,
    severity: Option<String>,
    title: Option<String>,
    description: Option<String>,
    recommendation: Option<String>,
    queue_item_id: Option<String>,
) -> Result<(), String> {
    let severity = severity.as_deref().map(parse_severity).transpose()?;
    edit_result(
        &report_id,
        &service_id,
        queue_item_id.as_deref(),
        |result| {
            let finding = result
                .findings
                .get_mut(finding_index)
                .ok_or_else(|| format!("Finding index out of range: {}", finding_index))?;
            if let Some(sev) = severity {
                finding.severity = sev;
            }
            if let Some(t) = title {
                finding.title = t;
            }
            if let Some(d) = description {
                finding.description = d;
            }
            if let Some(r) = recommendation {
                finding.recommendation = Some(r);
            }
            Ok(())
        },
    )
}

/// Add a new finding to a service result in a report
#[tauri::command(async)]
#[specta::specta]
pub fn add_report_finding(
    report_id: String,
    service_id: String,
    severity: String,
    title: String,
    description: String,
    recommendation: Option<String>,
    queue_item_id: Option<String>,
) -> Result<(), String> {
    let severity = parse_severity(&severity)?;
    edit_result(
        &report_id,
        &service_id,
        queue_item_id.as_deref(),
        |result| {
            result.findings.push(ServiceFinding {
                severity,
                title,
                description,
                recommendation,
                data: None,
            });
            Ok(())
        },
    )
}

/// Remove a finding from a service result in a report
#[tauri::command(async)]
#[specta::specta]
pub fn remove_report_finding(
    report_id: String,
    service_id: String,
    finding_index: usize,
    queue_item_id: Option<String>,
) -> Result<(), String> {
    edit_result(
        &report_id,
        &service_id,
        queue_item_id.as_deref(),
        |result| {
            if finding_index >= result.findings.len() {
                return Err(format!("Finding index out of range: {}", finding_index));
            }
            result.findings.remove(finding_index);
            Ok(())
        },
    )
}

/// Set agent-generated executive summary on a report
#[tauri::command(async)]
#[specta::specta]
pub fn set_report_summary(report_id: String, summary: String) -> Result<(), String> {
    let mut report = load_report(&report_id)?;
    report.agent_summary = Some(summary);
    save_report(&report)
}

/// Set agent-generated analysis for a specific service result
#[tauri::command(async)]
#[specta::specta]
pub fn set_service_analysis(
    report_id: String,
    service_id: String,
    analysis: String,
    queue_item_id: Option<String>,
) -> Result<(), String> {
    edit_result(
        &report_id,
        &service_id,
        queue_item_id.as_deref(),
        |result| {
            result.agent_analysis = Some(analysis);
            Ok(())
        },
    )
}

/// Set the health score (0-100, rounded) on a report
#[tauri::command(async)]
#[specta::specta]
pub fn set_report_health_score(report_id: String, score: f64) -> Result<(), String> {
    if !score.is_finite() || !(0.0..=100.0).contains(&score) {
        return Err("Health score must be between 0 and 100".to_string());
    }
    let mut report = load_report(&report_id)?;
    report.health_score = Some(score.round() as u8);
    save_report(&report)
}

// =============================================================================
// Statistics
// =============================================================================

fn compute_report_statistics(report: &ServiceReport) -> ReportStatistics {
    let total_services = report.results.len();
    let passed = report.results.iter().filter(|r| r.success).count();
    let total_duration_ms = report.results.iter().map(|r| r.duration_ms).sum::<u64>();
    let avg_duration_ms = if total_services > 0 {
        total_duration_ms / total_services as u64
    } else {
        0
    };

    let mut counts = FindingSeverityCounts {
        info: 0,
        success: 0,
        warning: 0,
        error: 0,
        critical: 0,
    };
    for finding in report.results.iter().flat_map(|r| &r.findings) {
        match finding.severity {
            FindingSeverity::Info => counts.info += 1,
            FindingSeverity::Success => counts.success += 1,
            FindingSeverity::Warning => counts.warning += 1,
            FindingSeverity::Error => counts.error += 1,
            FindingSeverity::Critical => counts.critical += 1,
        }
    }
    let total_findings =
        counts.info + counts.success + counts.warning + counts.error + counts.critical;

    // Health score: start at base, penalize for issues, bonus for successes
    let raw_score: i32 = HEALTH_SCORE_BASE
        - (counts.critical as i32 * CRITICAL_FINDING_PENALTY)
        - (counts.error as i32 * ERROR_FINDING_PENALTY)
        - (counts.warning as i32 * WARNING_FINDING_PENALTY)
        + (counts.success as i32 * SUCCESS_FINDING_BONUS);

    ReportStatistics {
        total_services,
        passed,
        failed: total_services - passed,
        total_duration_ms,
        avg_duration_ms,
        slowest_service: report
            .results
            .iter()
            .max_by_key(|r| r.duration_ms)
            .map(|r| (r.service_id.clone(), r.duration_ms)),
        fastest_service: report
            .results
            .iter()
            .min_by_key(|r| r.duration_ms)
            .map(|r| (r.service_id.clone(), r.duration_ms)),
        findings_by_severity: counts,
        total_findings,
        health_score: raw_score.clamp(0, HEALTH_SCORE_BASE) as u8,
    }
}

/// Get computed statistics for a report
#[tauri::command(async)]
#[specta::specta]
pub fn get_report_statistics(report_id: String) -> Result<ReportStatistics, String> {
    Ok(compute_report_statistics(&load_report(&report_id)?))
}

// =============================================================================
// Plain-text export
// =============================================================================

const RULE_HEAVY: &str = "═══════════════════════════════════════════════════════";
const RULE_LIGHT: &str = "───────────────────────────────────────────────────────";

fn format_header(report: &ServiceReport) -> Vec<String> {
    let mut lines = vec![
        RULE_HEAVY.to_string(),
        "                   SERVICE REPORT                     ".to_string(),
        RULE_HEAVY.to_string(),
        String::new(),
        format!("Report ID:    {}", report.id),
        format!("Date:         {}", report.started_at),
    ];
    if let Some(ref name) = report.technician_name {
        lines.push(format!("Technician:   {}", name));
    }
    if let Some(ref name) = report.customer_name {
        lines.push(format!("Customer:     {}", name));
    }
    lines.push(format!("Status:       {:?}", report.status));
    if let Some(score) = report.health_score {
        lines.push(format!("Health Score: {}/100", score));
    }
    lines.push(String::new());

    if let Some(ref summary) = report.agent_summary {
        lines.push(RULE_LIGHT.to_string());
        lines.push("  SUMMARY".to_string());
        lines.push(RULE_LIGHT.to_string());
        lines.push(summary.clone());
        lines.push(String::new());
    }
    lines
}

fn format_statistics(stats: &ReportStatistics) -> Vec<String> {
    vec![
        RULE_LIGHT.to_string(),
        "  STATISTICS".to_string(),
        RULE_LIGHT.to_string(),
        format!("Services Run:     {}", stats.total_services),
        format!("Passed:           {}", stats.passed),
        format!("Failed:           {}", stats.failed),
        format!(
            "Total Duration:   {:.1}s",
            stats.total_duration_ms as f64 / MS_PER_SECOND
        ),
        format!(
            "Avg per Service:  {:.1}s",
            stats.avg_duration_ms as f64 / MS_PER_SECOND
        ),
        String::new(),
        format!(
            "Findings:  {} critical, {} errors, {} warnings, {} info, {} success",
            stats.findings_by_severity.critical,
            stats.findings_by_severity.error,
            stats.findings_by_severity.warning,
            stats.findings_by_severity.info,
            stats.findings_by_severity.success
        ),
        String::new(),
    ]
}

fn format_findings(results: &[ServiceResult]) -> Vec<String> {
    let definitions = services::get_all_definitions();
    let names: HashMap<&str, &ServiceDefinition> =
        definitions.iter().map(|d| (d.id.as_str(), d)).collect();

    let mut lines = Vec::new();
    for result in results {
        let service_name = names
            .get(result.service_id.as_str())
            .map(|d| d.name.as_str())
            .unwrap_or(&result.service_id);

        lines.push(RULE_LIGHT.to_string());
        lines.push(format!(
            "  {} — {}",
            service_name,
            if result.success { "PASSED" } else { "FAILED" }
        ));
        lines.push(format!(
            "  Duration: {:.1}s",
            result.duration_ms as f64 / MS_PER_SECOND
        ));
        lines.push(RULE_LIGHT.to_string());

        if let Some(ref err) = result.error {
            lines.push(format!("  ERROR: {}", err));
        }
        for finding in &result.findings {
            let sev = match finding.severity {
                FindingSeverity::Info => "INFO",
                FindingSeverity::Success => "OK",
                FindingSeverity::Warning => "WARN",
                FindingSeverity::Error => "ERROR",
                FindingSeverity::Critical => "CRITICAL",
            };
            lines.push(format!("  [{}] {}", sev, finding.title));
            if !finding.description.is_empty() {
                lines.push(format!("         {}", finding.description));
            }
            if let Some(ref rec) = finding.recommendation {
                lines.push(format!("         → {}", rec));
            }
        }
        if let Some(ref analysis) = result.agent_analysis {
            lines.push(String::new());
            lines.push(format!("  Analysis: {}", analysis));
        }
        lines.push(String::new());
    }
    lines
}

/// Render a report as plain text.
fn render_text_report(report: &ServiceReport) -> String {
    let mut lines = format_header(report);
    lines.extend(format_statistics(&compute_report_statistics(report)));
    lines.extend(format_findings(&report.results));
    lines.push(RULE_HEAVY.to_string());
    lines.push(format!(
        "Generated by RustService — {}",
        Utc::now().to_rfc3339()
    ));
    lines.push(RULE_HEAVY.to_string());
    lines.join("\n")
}

/// Export a report as a plain-text file in `data/reports/` and return its path
///
/// `file_name` is an optional bare file name (no folders); `.txt` is added if
/// missing. Printable PDF reports are produced by the Reports page instead.
#[tauri::command(async)]
#[specta::specta]
pub fn export_report_text(report_id: String, file_name: Option<String>) -> Result<String, String> {
    let report = load_report(&report_id)?;

    let name = match file_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        Some(n) => {
            if n.contains(['/', '\\', ':']) || n.contains("..") {
                return Err("file_name must be a plain file name without folders".to_string());
            }
            if n.to_ascii_lowercase().ends_with(".txt") {
                n.to_string()
            } else {
                format!("{}.txt", n)
            }
        }
        None => format!("{}.txt", report.id),
    };

    let dir = get_reports_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create reports dir: {}", e))?;
    let path = dir.join(name);
    fs::write(&path, render_text_report(&report))
        .map_err(|e| format!("Failed to write report: {}", e))?;

    Ok(path.to_string_lossy().to_string())
}
