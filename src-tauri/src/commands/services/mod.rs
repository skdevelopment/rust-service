//! Service runner commands
//!
//! Tauri commands for running diagnostic and maintenance services. The service
//! implementations live in `crate::services`; this module covers definitions
//! and presets, the run state machine (`runner`) and reports (`reports`).

pub(crate) mod reports;
pub(crate) mod runner;

use std::collections::HashMap;

use sysinfo::Disks;

use super::required_programs::validate_required_programs;
use super::settings::{get_settings, update_settings};
use crate::services;
use crate::types::{ServiceDefinition, ServicePreset};

/// Bytes per gigabyte (for display formatting)
const BYTES_PER_GB: f64 = 1_073_741_824.0;

/// A removable drive the USB stability test can target
#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UsbDrive {
    pub mount_point: String,
    pub name: String,
    pub total_space_gb: f64,
    pub available_space_gb: f64,
    pub file_system: String,
}

/// List connected removable USB drives for the USB stability test service
#[tauri::command(async)]
#[specta::specta]
pub fn list_usb_drives() -> Vec<UsbDrive> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| d.is_removable() && d.total_space() > 0)
        .map(|d| {
            let mount = d.mount_point().to_string_lossy().to_string();
            let name = d.name().to_string_lossy().to_string();
            let label = if name.is_empty() {
                "Removable Disk".to_string()
            } else {
                name
            };
            UsbDrive {
                mount_point: mount.trim_end_matches('\\').to_string(),
                name: label,
                total_space_gb: d.total_space() as f64 / BYTES_PER_GB,
                available_space_gb: d.available_space() as f64 / BYTES_PER_GB,
                file_system: d.file_system().to_string_lossy().to_string(),
            }
        })
        .collect()
}

/// Get all available service definitions
#[tauri::command(async)]
#[specta::specta]
pub fn get_service_definitions() -> Vec<ServiceDefinition> {
    services::get_all_definitions()
}

/// Get all service presets (built-in merged with custom from settings)
#[tauri::command(async)]
#[specta::specta]
pub fn get_service_presets() -> Vec<ServicePreset> {
    let mut presets = services::get_all_presets();

    if let Ok(settings) = get_settings() {
        for custom in settings.presets.custom_presets {
            // A custom preset with a built-in id overrides the built-in one
            match presets.iter().position(|p| p.id == custom.id) {
                Some(pos) => presets[pos] = custom,
                None => presets.push(custom),
            }
        }
    }

    presets
}

/// Save or update a service preset
#[tauri::command(async)]
#[specta::specta]
pub fn save_service_preset(preset: ServicePreset) -> Result<(), String> {
    update_settings(|settings| {
        let presets = &mut settings.presets.custom_presets;
        match presets.iter().position(|p| p.id == preset.id) {
            Some(pos) => presets[pos] = preset,
            None => presets.push(preset),
        }
        Ok(())
    })
    .map(|_| ())
}

/// Delete a custom service preset
#[tauri::command(async)]
#[specta::specta]
pub fn delete_service_preset(preset_id: String) -> Result<(), String> {
    update_settings(|settings| {
        let presets = &mut settings.presets.custom_presets;
        let before = presets.len();
        presets.retain(|p| p.id != preset_id);
        if presets.len() == before {
            return Err(format!("Custom preset not found: {}", preset_id));
        }
        Ok(())
    })
    .map(|_| ())
}

/// Get the list of built-in preset IDs
#[tauri::command(async)]
#[specta::specta]
pub fn get_builtin_preset_ids() -> Vec<String> {
    services::get_all_presets()
        .into_iter()
        .map(|p| p.id)
        .collect()
}

/// Check if required programs are installed for given services
///
/// Returns a map of service id -> missing program ids (only services with
/// missing programs are included).
#[tauri::command(async)]
#[specta::specta]
pub fn validate_service_requirements(
    service_ids: Vec<String>,
) -> Result<HashMap<String, Vec<String>>, String> {
    let definitions = services::get_all_definitions();
    let def_map: HashMap<_, _> = definitions.iter().map(|d| (d.id.as_str(), d)).collect();

    let mut all_required: Vec<String> = Vec::new();
    for def in service_ids.iter().filter_map(|id| def_map.get(id.as_str())) {
        for req in &def.required_programs {
            if !all_required.contains(req) {
                all_required.push(req.clone());
            }
        }
    }

    let validation = validate_required_programs(all_required)?;

    let mut missing: HashMap<String, Vec<String>> = HashMap::new();
    for service_id in service_ids {
        let Some(def) = def_map.get(service_id.as_str()) else {
            continue;
        };
        let service_missing: Vec<String> = def
            .required_programs
            .iter()
            .filter(|req| !validation.get(*req).copied().unwrap_or(false))
            .cloned()
            .collect();
        if !service_missing.is_empty() {
            missing.insert(service_id, service_missing);
        }
    }

    Ok(missing)
}
