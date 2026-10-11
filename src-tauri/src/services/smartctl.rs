//! Smartctl Drive Health Service
//!
//! Uses smartctl from smartmontools to report SMART data for all drives.
//! Provides health status, wear level, power-on hours, and other metrics.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::commands::required_programs::get_program_exe_path;
use crate::services::{Service, ServiceCtx};
use crate::types::{FindingSeverity, ServiceDefinition, ServiceFinding, ServiceResult};

// =============================================================================
// Service Implementation
// =============================================================================

pub struct SmartctlService;

impl Service for SmartctlService {
    fn definition(&self) -> ServiceDefinition {
        ServiceDefinition {
            id: "smartctl".to_string(),
            name: "Drive Health Report".to_string(),
            description: "Reports SMART health data for all drives".to_string(),
            category: "diagnostics".to_string(),
            estimated_duration_secs: 30,
            required_programs: vec!["smartctl".to_string()],
            options: vec![],
            icon: "activity".to_string(),
            exclusive_resources: vec![],
            dependencies: vec![],
        }
    }

    fn run(&self, _options: &serde_json::Value, mut ctx: ServiceCtx) -> ServiceResult {
        let mut success = true;
        let mut error: Option<String> = None;

        ctx.log("Starting drive health analysis...");

        // Get executable path
        let exe_path = match get_program_exe_path("smartctl".to_string()) {
            Ok(Some(path)) => path,
            Ok(None) => {
                return ctx.finish(false, Some("smartctl executable not found".to_string()));
            }
            Err(e) => {
                return ctx.finish(false, Some(format!("Failed to get executable path: {}", e)));
            }
        };

        ctx.log(format!("Using smartctl: {}", exe_path));

        // Scan for devices
        ctx.log("Scanning for drives...");
        let scan_output = ctx.output(crate::proc::command(&exe_path).args(["--scan", "-j"]));

        let devices = match scan_output {
            Ok(output) => {
                let stdout = crate::proc::decode_output(&output.stdout);
                match serde_json::from_str::<ScanResult>(&stdout) {
                    Ok(scan) => scan.devices.unwrap_or_default(),
                    Err(e) => {
                        ctx.log(format!("Warning: Failed to parse scan output: {}", e));
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                success = false;
                error = Some(format!("Failed to scan devices: {}", e));
                ctx.log(format!("Error scanning devices: {}", e));
                Vec::new()
            }
        };

        if devices.is_empty() {
            ctx.log("No drives found or failed to enumerate drives");
        }

        ctx.log(format!("Found {} drive(s)", devices.len()));

        let mut drives_data: Vec<DriveHealth> = Vec::new();
        let mut healthy_count = 0;
        let mut warning_count = 0;
        let mut failed_count = 0;

        for device in &devices {
            let device_name = &device.name;

            // Skip USB devices
            if device
                .r#type
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("usb")
                || device
                    .protocol
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("usb")
            {
                ctx.log(format!("Skipping USB device: {}", device_name));
                continue;
            }

            ctx.log(format!("Querying drive: {}", device_name));

            let info_output =
                ctx.output(crate::proc::command(&exe_path).args(["-a", device_name, "-j"]));

            if let Ok(output) = info_output {
                let stdout = crate::proc::decode_output(&output.stdout);
                let stderr = crate::proc::decode_output(&output.stderr);

                // Skip if stderr indicates USB bridge issues
                if stderr.to_lowercase().contains("usb")
                    || stderr.to_lowercase().contains("please specify device type")
                {
                    ctx.log(format!("Skipping {} (USB bridge detected)", device_name));
                    continue;
                }

                if let Ok(info) = serde_json::from_str::<DeviceInfo>(&stdout) {
                    let health = build_drive_health(&info, device_name);

                    if health.health_passed == Some(true) {
                        healthy_count += 1;
                    } else if health.health_passed == Some(false) {
                        failed_count += 1;
                    } else if health.wear_level_percent.map(|w| w > 80).unwrap_or(false) {
                        warning_count += 1;
                    } else {
                        healthy_count += 1;
                    }

                    ctx.log(format!(
                        "  {} - {} ({})",
                        health.model_name,
                        if health.health_passed == Some(true) {
                            "Healthy"
                        } else if health.health_passed == Some(false) {
                            "FAILED"
                        } else {
                            "Unknown"
                        },
                        health.serial_number.as_deref().unwrap_or("N/A")
                    ));

                    drives_data.push(health);
                }
            }
        }

        ctx.log(format!(
            "Analysis complete: {} healthy, {} warning, {} failed",
            healthy_count, warning_count, failed_count
        ));

        // Create findings
        let overall_severity = if failed_count > 0 {
            FindingSeverity::Error
        } else if warning_count > 0 {
            FindingSeverity::Warning
        } else if healthy_count > 0 {
            FindingSeverity::Success
        } else {
            FindingSeverity::Info
        };

        let overall_title = if failed_count > 0 {
            format!("{} Drive(s) Have Health Issues", failed_count)
        } else if warning_count > 0 {
            format!("{} Drive(s) Show Wear", warning_count)
        } else if healthy_count > 0 {
            format!("All {} Drive(s) Healthy", healthy_count)
        } else {
            "No Drives Analyzed".to_string()
        };

        ctx.findings.push(ServiceFinding {
            severity: overall_severity,
            title: overall_title,
            description: format!("Analyzed {} drive(s) using SMART data.", drives_data.len()),
            recommendation: if failed_count > 0 {
                Some("Backup data immediately and consider replacing failing drives.".to_string())
            } else if warning_count > 0 {
                Some("Monitor drive health and plan for replacement.".to_string())
            } else {
                None
            },
            data: Some(json!({
                "type": "smartctl_result",
                "drives": drives_data,
                "summary": {
                    "total": drives_data.len(),
                    "healthy": healthy_count,
                    "warning": warning_count,
                    "failed": failed_count,
                }
            })),
        });

        ctx.finish(success, error)
    }
}

// =============================================================================
// Data Structures
// =============================================================================

#[derive(Debug, Deserialize, specta::Type)]
struct ScanResult {
    devices: Option<Vec<ScannedDevice>>,
}

#[derive(Debug, Deserialize, specta::Type)]
struct ScannedDevice {
    name: String,
    r#type: Option<String>,
    protocol: Option<String>,
}

#[derive(Debug, Deserialize, specta::Type)]
struct DeviceInfo {
    device: Option<DeviceField>,
    model_name: Option<String>,
    serial_number: Option<String>,
    firmware_version: Option<String>,
    smart_status: Option<SmartStatus>,
    nvme_smart_health_information_log: Option<NvmeHealthLog>,
    power_on_time: Option<PowerOnTime>,
    power_cycle_count: Option<u64>,
    temperature: Option<Temperature>,
}

#[derive(Debug, Deserialize, specta::Type)]
struct DeviceField {
    name: Option<String>,
}

#[derive(Debug, Deserialize, specta::Type)]
struct SmartStatus {
    passed: Option<bool>,
}

#[derive(Debug, Deserialize, specta::Type)]
struct NvmeHealthLog {
    percentage_used: Option<u8>,
    power_cycles: Option<u64>,
    power_on_hours: Option<u64>,
    unsafe_shutdowns: Option<u64>,
    media_errors: Option<u64>,
    _num_err_log_entries: Option<u64>,
    data_units_written: Option<u64>,
    data_units_read: Option<u64>,
    temperature_sensors: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize, specta::Type)]
struct PowerOnTime {
    hours: Option<u64>,
}

#[derive(Debug, Deserialize, specta::Type)]
struct Temperature {
    current: Option<i32>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct DriveHealth {
    name: String,
    model_name: String,
    serial_number: Option<String>,
    firmware_version: Option<String>,
    health_passed: Option<bool>,
    wear_level_percent: Option<u8>,
    power_on_hours: Option<u64>,
    power_cycles: Option<u64>,
    unsafe_shutdowns: Option<u64>,
    media_errors: Option<u64>,
    data_written_tb: Option<f64>,
    data_read_tb: Option<f64>,
    temperature_celsius: Option<i32>,
}

fn build_drive_health(info: &DeviceInfo, device_name: &str) -> DriveHealth {
    let name = info
        .device
        .as_ref()
        .and_then(|d| d.name.clone())
        .unwrap_or_else(|| device_name.to_string());

    let model = info
        .model_name
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let serial = info.serial_number.clone();
    let firmware = info.firmware_version.clone();
    let health_passed = info.smart_status.as_ref().and_then(|s| s.passed);

    // NVMe specific data
    let nvme = &info.nvme_smart_health_information_log;
    let wear_level = nvme.as_ref().and_then(|n| n.percentage_used);
    let power_on_hours = nvme
        .as_ref()
        .and_then(|n| n.power_on_hours)
        .or_else(|| info.power_on_time.as_ref().and_then(|p| p.hours));
    let power_cycles = nvme
        .as_ref()
        .and_then(|n| n.power_cycles)
        .or(info.power_cycle_count);
    let unsafe_shutdowns = nvme.as_ref().and_then(|n| n.unsafe_shutdowns);
    let media_errors = nvme.as_ref().and_then(|n| n.media_errors);

    // Data written/read (NVMe data units = 512KB * 1000 = 512000 bytes)
    let data_written_tb = nvme
        .as_ref()
        .and_then(|n| n.data_units_written)
        .map(|units| (units as f64 * 512000.0) / 1_000_000_000_000.0);
    let data_read_tb = nvme
        .as_ref()
        .and_then(|n| n.data_units_read)
        .map(|units| (units as f64 * 512000.0) / 1_000_000_000_000.0);

    // Temperature
    let temp = nvme
        .as_ref()
        .and_then(|n| n.temperature_sensors.as_ref())
        .and_then(|temps| temps.first().copied())
        .or_else(|| info.temperature.as_ref().and_then(|t| t.current));

    DriveHealth {
        name,
        model_name: model,
        serial_number: serial,
        firmware_version: firmware,
        health_passed,
        wear_level_percent: wear_level,
        power_on_hours,
        power_cycles,
        unsafe_shutdowns,
        media_errors,
        data_written_tb,
        data_read_tb,
        temperature_celsius: temp,
    }
}
