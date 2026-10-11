//! Modular service system
//!
//! Each service is implemented in its own file and registered here.
//! Services implement the `Service` trait and do their work through a
//! [`ServiceCtx`], which handles logging, findings, cancellation and results.

mod adwcleaner;
mod battery_report;
mod bleachbit;
mod chkdsk;
mod ctx;
mod disk_space;
mod dism;
mod drivecleanup;
mod driver_audit;
mod energy_report;
mod furmark;
mod heavyload;
mod installed_software;
mod iperf;
mod kvrt_scan;
mod network_config;
mod ping_test;
mod restore_point;
mod sfc;
mod smartctl;
mod speedtest;
mod startup_optimize;
mod stinger;
mod usb_stability;
mod whynotwin11;
mod windows_update;
mod winsat;

use serde_json::json;
use std::sync::LazyLock;
use tauri::AppHandle;

pub use ctx::{cancel_requested, request_cancel, reset_cancel, ServiceCtx};

use crate::types::{PresetServiceConfig, ServiceDefinition, ServicePreset, ServiceResult};

// =============================================================================
// Service Trait
// =============================================================================

/// Trait that all services must implement
pub trait Service: Send + Sync {
    /// Returns the service definition (id, name, description, options, etc.)
    fn definition(&self) -> ServiceDefinition;

    /// Executes the service with the given options
    fn run(&self, options: &serde_json::Value, ctx: ServiceCtx) -> ServiceResult;
}

// =============================================================================
// Service Registry
// =============================================================================

/// Static registry of all available services, in display order
static SERVICE_REGISTRY: LazyLock<Vec<(String, Box<dyn Service>)>> = LazyLock::new(|| {
    let services: Vec<Box<dyn Service>> = vec![
        Box::new(restore_point::RestorePointService),
        Box::new(ping_test::PingTestService),
        Box::new(disk_space::DiskSpaceService),
        Box::new(winsat::WinsatService),
        Box::new(kvrt_scan::KvrtScanService),
        Box::new(adwcleaner::AdwCleanerService),
        Box::new(stinger::StingerService),
        Box::new(whynotwin11::WhyNotWin11Service),
        Box::new(smartctl::SmartctlService),
        Box::new(speedtest::SpeedtestService),
        Box::new(iperf::IperfService),
        Box::new(bleachbit::BleachBitService),
        Box::new(drivecleanup::DriveCleanupService),
        Box::new(heavyload::HeavyLoadService),
        Box::new(dism::DismService),
        Box::new(sfc::SfcService),
        Box::new(chkdsk::ChkdskService),
        Box::new(furmark::FurmarkService),
        Box::new(windows_update::WindowsUpdateService),
        Box::new(energy_report::EnergyReportService),
        Box::new(battery_report::BatteryReportService),
        Box::new(driver_audit::DriverAuditService),
        Box::new(installed_software::InstalledSoftwareService),
        Box::new(network_config::NetworkConfigService),
        Box::new(usb_stability::UsbStabilityService),
        Box::new(startup_optimize::StartupOptimizeService),
    ];

    services
        .into_iter()
        .map(|s| (s.definition().id.clone(), s))
        .collect()
});

/// Get all service definitions (stable, registration order)
pub fn get_all_definitions() -> Vec<ServiceDefinition> {
    SERVICE_REGISTRY
        .iter()
        .map(|(_, s)| s.definition())
        .collect()
}

/// Get a service by ID
pub fn get_service(id: &str) -> Option<&'static dyn Service> {
    SERVICE_REGISTRY
        .iter()
        .find(|(sid, _)| sid == id)
        .map(|(_, s)| s.as_ref())
}

/// Run a service by ID
pub fn run_service(
    id: &str,
    options: &serde_json::Value,
    app: &AppHandle,
) -> Option<ServiceResult> {
    get_service(id).map(|s| s.run(options, ServiceCtx::new(app, id)))
}

// =============================================================================
// Preset Definitions
// =============================================================================

fn svc(service_id: &str, enabled: bool, options: serde_json::Value) -> PresetServiceConfig {
    PresetServiceConfig {
        service_id: service_id.to_string(),
        enabled,
        options,
    }
}

/// Get all service presets
pub fn get_all_presets() -> Vec<ServicePreset> {
    vec![
        ServicePreset {
            id: "diagnostics".to_string(),
            name: "Diagnostics".to_string(),
            description: "Quick diagnostic tests to identify system issues".to_string(),
            services: vec![
                svc("ping-test", true, json!({"target": "8.8.8.8", "count": 4})),
                svc("speedtest", true, json!({})),
                svc("disk-space", true, json!({})),
                svc("smartctl", true, json!({})),
                svc("battery-report", true, json!({})),
                svc("whynotwin11", true, json!({})),
                svc("energy-report", true, json!({"duration": 10})),
                svc("network-config", true, json!({})),
            ],
            icon: "stethoscope".to_string(),
            color: "blue".to_string(),
        },
        ServicePreset {
            id: "general".to_string(),
            name: "General Service".to_string(),
            description: "Standard maintenance tasks for regular checkups".to_string(),
            services: vec![
                svc("restore-point", true, json!({})),
                svc("adwcleaner", true, json!({})),
                svc("kvrt-scan", true, json!({})),
                svc("ping-test", true, json!({"target": "8.8.8.8", "count": 4})),
                svc("speedtest", true, json!({})),
                svc("disk-space", true, json!({})),
                svc("smartctl", true, json!({})),
                svc("battery-report", true, json!({})),
                svc("network-config", true, json!({})),
            ],
            icon: "wrench".to_string(),
            color: "green".to_string(),
        },
        ServicePreset {
            id: "complete".to_string(),
            name: "Complete Service".to_string(),
            description: "Comprehensive scan and cleanup for thorough maintenance".to_string(),
            services: vec![
                svc("restore-point", true, json!({})),
                svc("adwcleaner", true, json!({})),
                svc("kvrt-scan", true, json!({})),
                svc("bleachbit", true, json!({})),
                svc("drivecleanup", true, json!({"test_only": false})),
                svc("ping-test", true, json!({"target": "8.8.8.8", "count": 10})),
                svc("speedtest", true, json!({})),
                svc(
                    "iperf",
                    true,
                    json!({"server": "iperf.he.net", "duration": 60, "reverse": true}),
                ),
                svc("disk-space", true, json!({})),
                svc("smartctl", true, json!({})),
                svc("battery-report", true, json!({})),
                svc("whynotwin11", true, json!({})),
                svc("winsat", true, json!({"drive": "C"})),
                svc("dism", true, json!({"scan_only": false})),
                svc("sfc", true, json!({})),
                svc(
                    "windows-update",
                    true,
                    json!({"install_updates": true, "include_drivers": true}),
                ),
                svc("energy-report", true, json!({"duration": 10})),
                svc("driver-audit", true, json!({"show_all": false})),
                svc(
                    "installed-software",
                    true,
                    json!({"include_updates": false}),
                ),
                svc("network-config", true, json!({})),
                svc(
                    "usb-stability",
                    false,
                    json!({"test_intensity": "standard", "verify_integrity": true}),
                ),
                svc(
                    "startup-optimize",
                    true,
                    json!({"disable_unnecessary": false, "include_disabled": false}),
                ),
            ],
            icon: "shield-check".to_string(),
            color: "purple".to_string(),
        },
        ServicePreset {
            id: "custom".to_string(),
            name: "Custom Service".to_string(),
            description: "Build your own service configuration".to_string(),
            services: vec![
                svc("restore-point", false, json!({})),
                svc("adwcleaner", false, json!({})),
                svc("kvrt-scan", false, json!({})),
                svc("bleachbit", false, json!({})),
                svc("drivecleanup", false, json!({"test_only": true})),
                svc("ping-test", false, json!({})),
                svc("speedtest", false, json!({})),
                svc(
                    "iperf",
                    false,
                    json!({"server": "iperf.he.net", "duration": 30, "reverse": true}),
                ),
                svc("disk-space", false, json!({})),
                svc("smartctl", false, json!({})),
                svc("battery-report", false, json!({})),
                svc("whynotwin11", false, json!({})),
                svc("winsat", false, json!({"drive": "C"})),
                svc(
                    "heavyload",
                    false,
                    json!({"duration_minutes": 5, "stress_cpu": true, "stress_memory": true, "stress_disk": false}),
                ),
                svc("dism", false, json!({"scan_only": true})),
                svc("sfc", false, json!({})),
                svc(
                    "windows-update",
                    false,
                    json!({"install_updates": false, "include_drivers": true}),
                ),
                svc(
                    "chkdsk",
                    false,
                    json!({"drive": "C:", "mode": "read_only", "schedule_if_busy": false}),
                ),
                svc(
                    "furmark",
                    false,
                    json!({"duration_seconds": 60, "width": 1920, "height": 1080}),
                ),
                svc(
                    "stinger",
                    false,
                    json!({"action": "report", "include_pups": false}),
                ),
                svc("energy-report", false, json!({"duration": 10})),
                svc("driver-audit", false, json!({"show_all": false})),
                svc(
                    "installed-software",
                    false,
                    json!({"include_updates": false}),
                ),
                svc("network-config", false, json!({"include_disabled": false})),
                svc(
                    "usb-stability",
                    false,
                    json!({"test_intensity": "standard", "verify_integrity": true}),
                ),
                svc(
                    "startup-optimize",
                    false,
                    json!({"disable_unnecessary": false, "include_disabled": false}),
                ),
            ],
            icon: "settings-2".to_string(),
            color: "orange".to_string(),
        },
    ]
}
