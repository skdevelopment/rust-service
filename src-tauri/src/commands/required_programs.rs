//! Required programs commands
//!
//! Tauri commands for managing required external programs.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use super::data_dir::get_data_dir_path;
use super::settings::get_settings;
use crate::types::{
    RequiredProgramDef, RequiredProgramInstallPhase, RequiredProgramInstallProgress,
    RequiredProgramStatus,
};

/// Event the service dialog and Settings → Programs both listen to.
const INSTALL_PROGRESS_EVENT: &str = "required-program-install-progress";
const DOWNLOAD_CHUNK: usize = 64 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// Program ids whose install is currently running. A second click attaches on
/// the frontend; this stops a second download from writing the same folder.
static IN_FLIGHT: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Debug)]
struct InstallGuard(String);

impl Drop for InstallGuard {
    fn drop(&mut self) {
        if let Ok(mut in_flight) = IN_FLIGHT.lock() {
            in_flight.remove(&self.0);
        }
    }
}

fn begin_install(id: &str) -> Result<InstallGuard, String> {
    let mut in_flight = IN_FLIGHT
        .lock()
        .map_err(|e| format!("Install lock poisoned: {e}"))?;
    if !in_flight.insert(id.to_string()) {
        let name = REQUIRED_PROGRAMS
            .get(id)
            .map(|def| def.name.as_str())
            .unwrap_or(id);
        return Err(format!("{name} is already installing"));
    }
    Ok(InstallGuard(id.to_string()))
}

// =============================================================================
// Required Programs Registry
// =============================================================================

/// Static registry of all known required programs
static REQUIRED_PROGRAMS: LazyLock<HashMap<String, RequiredProgramDef>> = LazyLock::new(|| {
    let programs = vec![
        RequiredProgramDef {
            id: "bleachbit".to_string(),
            name: "BleachBit".to_string(),
            description: "System cleaner to free disk space and maintain privacy".to_string(),
            exe_names: vec![
                "bleachbit.exe".to_string(),
                "bleachbit_console.exe".to_string(),
            ],
            url: Some("https://www.bleachbit.org/".to_string()),
            download_url: Some("https://download.bleachbit.org/BleachBit-4.6.2-portable.zip".to_string()),
        },
        RequiredProgramDef {
            id: "adwcleaner".to_string(),
            name: "AdwCleaner".to_string(),
            description: "Adware and malware removal tool".to_string(),
            exe_names: vec!["adwcleaner.exe".to_string()],
            url: Some("https://www.malwarebytes.com/adwcleaner".to_string()),
            download_url: Some("https://adwcleaner.malwarebytes.com/adwcleaner?channel=release".to_string()),
        },
        RequiredProgramDef {
            id: "crystaldiskinfo".to_string(),
            name: "CrystalDiskInfo".to_string(),
            description: "Disk health monitoring utility".to_string(),
            exe_names: vec![
                "DiskInfo64.exe".to_string(),
                "DiskInfo32.exe".to_string(),
                "CrystalDiskInfo.exe".to_string(),
            ],
            url: Some("https://crystalmark.info/en/software/crystaldiskinfo/".to_string()),
            download_url: Some("https://sourceforge.net/projects/crystaldiskinfo/files/latest/download".to_string()),
        },
        RequiredProgramDef {
            id: "kvrt".to_string(),
            name: "Kaspersky Virus Removal Tool".to_string(),
            description: "Antivirus scanning and malware removal tool".to_string(),
            exe_names: vec!["KVRT.exe".to_string()],
            url: Some("https://www.kaspersky.com/downloads/free-virus-removal-tool".to_string()),
            download_url: None,
        },
        RequiredProgramDef {
            id: "whynotwin11".to_string(),
            name: "WhyNotWin11".to_string(),
            description: "Windows 11 compatibility checker".to_string(),
            exe_names: vec![
                "WhyNotWin11.exe".to_string(),
                "WhyNotWin11Portable.exe".to_string(),
            ],
            url: Some("https://github.com/rcmaehl/WhyNotWin11/releases".to_string()),
            download_url: Some("https://github.com/rcmaehl/WhyNotWin11/releases/latest/download/WhyNotWin11.exe".to_string()),
        },
        RequiredProgramDef {
            id: "smartctl".to_string(),
            name: "Smartctl (smartmontools)".to_string(),
            description: "SMART disk health monitoring and reporting tool".to_string(),
            exe_names: vec!["smartctl.exe".to_string()],
            url: Some("https://github.com/smartmontools/smartmontools/releases".to_string()),
            // Resolved at install time to the latest official Windows build.
            download_url: Some(
                "https://github.com/smartmontools/smartmontools/releases/latest".to_string(),
            ),
        },
        RequiredProgramDef {
            id: "speedtest".to_string(),
            name: "Speedtest CLI".to_string(),
            description: "Ookla Speedtest command-line interface for bandwidth testing".to_string(),
            exe_names: vec!["speedtest.exe".to_string()],
            url: Some("https://www.speedtest.net/apps/cli".to_string()),
            download_url: Some("https://install.speedtest.net/app/cli/ookla-speedtest-1.2.0-win64.zip".to_string()),
        },
        RequiredProgramDef {
            id: "iperf3".to_string(),
            name: "iPerf3".to_string(),
            description: "Network performance and stability testing tool".to_string(),
            exe_names: vec!["iperf3.exe".to_string()],
            url: Some("https://iperf.fr/iperf-download.php".to_string()),
            download_url: Some("https://github.com/ar51an/iperf3-win-builds/releases/download/3.21/iperf-3.21-win64.zip".to_string()),
        },
        RequiredProgramDef {
            id: "drivecleanup".to_string(),
            name: "DriveCleanup".to_string(),
            description: "Remove stale USB devices and registry entries".to_string(),
            exe_names: vec!["DriveCleanup.exe".to_string()],
            url: Some("https://www.uwe-sieber.de/drivetools_e.html".to_string()),
            download_url: Some("https://www.uwe-sieber.de/files/DriveCleanup.zip".to_string()),
        },
        RequiredProgramDef {
            id: "heavyload".to_string(),
            name: "HeavyLoad".to_string(),
            description: "CPU, memory, and disk stress testing tool".to_string(),
            exe_names: vec!["HeavyLoad.exe".to_string()],
            url: Some("https://www.jam-software.com/heavyload".to_string()),
            download_url: Some("https://downloads.jam-software.de/heavyload/HeavyLoad-x64.zip".to_string()),
        },
        RequiredProgramDef {
            id: "furmark".to_string(),
            name: "FurMark".to_string(),
            description: "GPU stress testing and stability tool".to_string(),
            exe_names: vec!["FurMark.exe".to_string(), "FurMark_GUI.exe".to_string()],
            url: Some("https://geeks3d.com/furmark/".to_string()),
            download_url: Some("https://gpumagick.com/downloads/files/2025/fm2/2_10_dbc69dd0a08da5ff09169a4fc759ddaa/FurMark_2.10.2_win64.zip".to_string()),
        },
        RequiredProgramDef {
            id: "stinger".to_string(),
            name: "Trellix Stinger".to_string(),
            description: "Standalone antivirus scanner for malware detection".to_string(),
            exe_names: vec![
                "stinger64.exe".to_string(),
                "stinger32.exe".to_string(),
                "stinger.exe".to_string(),
            ],
            url: Some("https://www.trellix.com/products/stinger/".to_string()),
            download_url: Some("https://downloadcenter.trellix.com/products/mcafee-avert/Stinger/stinger64.exe".to_string()),
        },
        // Add more programs here as services require them
    ];

    programs.into_iter().map(|p| (p.id.clone(), p)).collect()
});

// =============================================================================
// Helper Functions
// =============================================================================

/// Search for an executable in the data/programs folder recursively
fn find_exe_in_programs_folder(exe_names: &[String]) -> Option<PathBuf> {
    let programs_dir = get_data_dir_path().join("programs");
    if !programs_dir.exists() {
        return None;
    }

    find_exe_recursive(&programs_dir, exe_names)
}

fn find_exe_recursive(dir: &PathBuf, exe_names: &[String]) -> Option<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_exe_recursive(&path, exe_names) {
                return Some(found);
            }
        } else if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let file_lower = file_name.to_lowercase();
                for exe_name in exe_names {
                    if file_lower == exe_name.to_lowercase() {
                        return Some(path);
                    }
                }
            }
        }
    }

    None
}

// =============================================================================
// Tauri Commands
// =============================================================================

/// Get all required program definitions
#[tauri::command(async)]
#[specta::specta]
pub fn get_required_programs() -> Vec<RequiredProgramDef> {
    REQUIRED_PROGRAMS.values().cloned().collect()
}

/// Get required program definitions for specific IDs only
#[tauri::command(async)]
#[specta::specta]
pub fn get_required_programs_by_ids(ids: Vec<String>) -> Vec<RequiredProgramDef> {
    ids.iter()
        .filter_map(|id| REQUIRED_PROGRAMS.get(id).cloned())
        .collect()
}

/// Get the status of all required programs (found/not found, paths)
#[tauri::command]
#[specta::specta]
pub async fn get_required_programs_status() -> Result<Vec<RequiredProgramStatus>, String> {
    tokio::task::spawn_blocking(get_required_programs_status_blocking)
        .await
        .map_err(|e| format!("Required programs task failed: {e}"))?
}

pub(crate) fn get_required_programs_status_blocking() -> Result<Vec<RequiredProgramStatus>, String>
{
    let settings = get_settings()?;
    let overrides = &settings.programs.overrides;

    let mut statuses: Vec<RequiredProgramStatus> = Vec::new();

    for def in REQUIRED_PROGRAMS.values() {
        // Check for user override first
        if let Some(custom_path) = overrides.get(&def.id) {
            let path = PathBuf::from(custom_path);
            let found = path.exists() && path.is_file();
            statuses.push(RequiredProgramStatus {
                definition: def.clone(),
                found,
                path: Some(custom_path.clone()),
                is_custom: true,
            });
            continue;
        }

        // Auto-detect in programs folder
        if let Some(found_path) = find_exe_in_programs_folder(&def.exe_names) {
            statuses.push(RequiredProgramStatus {
                definition: def.clone(),
                found: true,
                path: Some(found_path.to_string_lossy().to_string()),
                is_custom: false,
            });
        } else {
            statuses.push(RequiredProgramStatus {
                definition: def.clone(),
                found: false,
                path: None,
                is_custom: false,
            });
        }
    }

    // Sort by name for consistent display
    statuses.sort_by(|a, b| a.definition.name.cmp(&b.definition.name));

    Ok(statuses)
}

/// Set a custom path override for a required program
#[tauri::command(async)]
#[specta::specta]
pub fn set_program_path_override(program_id: String, path: Option<String>) -> Result<(), String> {
    super::settings::update_settings(|settings| {
        match path {
            Some(p) if !p.trim().is_empty() => {
                settings.programs.overrides.insert(program_id, p);
            }
            _ => {
                settings.programs.overrides.remove(&program_id);
            }
        }
        Ok(())
    })
    .map(|_| ())
}

/// Get the resolved executable path for a required program (for service execution)
#[tauri::command(async)]
#[specta::specta]
pub fn get_program_exe_path(program_id: String) -> Result<Option<String>, String> {
    let settings = get_settings()?;

    // Check override first
    if let Some(custom_path) = settings.programs.overrides.get(&program_id) {
        let path = PathBuf::from(custom_path);
        if path.exists() && path.is_file() {
            return Ok(Some(custom_path.clone()));
        }
    }

    // Auto-detect
    if let Some(def) = REQUIRED_PROGRAMS.get(&program_id) {
        if let Some(found_path) = find_exe_in_programs_folder(&def.exe_names) {
            return Ok(Some(found_path.to_string_lossy().to_string()));
        }
    }

    Ok(None)
}

/// Validate that required programs are available for given service IDs
#[tauri::command(async)]
#[specta::specta]
pub fn validate_required_programs(
    required_ids: Vec<String>,
) -> Result<HashMap<String, bool>, String> {
    let settings = get_settings()?;
    let overrides = &settings.programs.overrides;

    let mut result: HashMap<String, bool> = HashMap::new();

    for id in required_ids {
        // Check override first
        if let Some(custom_path) = overrides.get(&id) {
            let path = PathBuf::from(custom_path);
            result.insert(id, path.exists() && path.is_file());
            continue;
        }

        // Auto-detect
        if let Some(def) = REQUIRED_PROGRAMS.get(&id) {
            let found = find_exe_in_programs_folder(&def.exe_names).is_some();
            result.insert(id, found);
        } else {
            // Unknown program ID
            result.insert(id, false);
        }
    }

    Ok(result)
}

/// One-click install: download the pinned payload and place it under `data/programs/{id}`.
///
/// Progress is emitted on [`INSTALL_PROGRESS_EVENT`] and is not tied to the
/// dialog that started the install. Closing that dialog does not cancel the work.
#[tauri::command]
#[specta::specta]
pub async fn install_required_program(
    app: AppHandle,
    id: String,
) -> Result<RequiredProgramStatus, String> {
    crate::proc::blocking(move || install_required_program_blocking(&app, &id)).await
}

/// Download `url` and install it as required program `id` (testable unit).
#[cfg(test)]
pub fn install_required_program_from_url(id: &str, url: &str) -> Result<PathBuf, String> {
    install_required_program_from_url_with_progress(id, url, |_, _| {})
}

#[cfg(test)]
fn install_required_program_from_url_with_progress(
    id: &str,
    url: &str,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf, String> {
    let (filename, bytes) = download_required_program(id, url, &mut on_progress)?;
    install_required_program_bytes(id, &filename, &bytes)
}

fn download_required_program(
    id: &str,
    url: &str,
    on_progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<(String, Vec<u8>), String> {
    let def = REQUIRED_PROGRAMS
        .get(id)
        .ok_or_else(|| format!("Unknown required program: {id}"))?;
    if url.trim().is_empty() {
        return Err(format!(
            "No unattended download is available for {}",
            def.name
        ));
    }
    let resolved = resolve_download_url(id, url)?;
    let url = resolved.as_str();

    let response = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?
        .get(url)
        .header("User-Agent", "RustService")
        .send()
        .map_err(|e| format!("Failed to download {}: {e}", def.name))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download {}: HTTP {}",
            def.name,
            response.status()
        ));
    }

    let filename = filename_from_response(&response, url);
    let bytes = read_body_with_progress(response, on_progress)?;
    Ok((filename, bytes))
}

fn read_body_with_progress(
    mut response: reqwest::blocking::Response,
    on_progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<Vec<u8>, String> {
    let total = response.content_length();
    let mut body = Vec::new();
    if let Some(total) = total.filter(|n| *n <= 512 * 1024 * 1024) {
        body.reserve(total as usize);
    }
    let mut received: u64 = 0;
    let mut buf = [0u8; DOWNLOAD_CHUNK];
    let mut last_emit = Instant::now() - PROGRESS_INTERVAL;
    on_progress(0, total);
    loop {
        let n = response
            .read(&mut buf)
            .map_err(|e| format!("Failed to read download body: {e}"))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        received += n as u64;
        if last_emit.elapsed() >= PROGRESS_INTERVAL {
            on_progress(received, total);
            last_emit = Instant::now();
        }
    }
    on_progress(received, total);
    Ok(body)
}

fn install_required_program_blocking(
    app: &AppHandle,
    id: &str,
) -> Result<RequiredProgramStatus, String> {
    let _guard = begin_install(id)?;
    match install_required_program_body(app, id) {
        Ok(status) => Ok(status),
        Err(message) => {
            emit_install_progress(
                app,
                RequiredProgramInstallProgress {
                    program_id: id.to_string(),
                    phase: RequiredProgramInstallPhase::Error,
                    bytes_received: 0,
                    bytes_total: None,
                    message: Some(message.clone()),
                },
            );
            Err(message)
        }
    }
}

fn install_required_program_body(
    app: &AppHandle,
    id: &str,
) -> Result<RequiredProgramStatus, String> {
    let def = REQUIRED_PROGRAMS
        .get(id)
        .ok_or_else(|| format!("Unknown required program: {id}"))?;
    let url = def.download_url.as_deref().unwrap_or("");
    let (filename, bytes) = download_required_program(id, url, &mut |received, total| {
        emit_install_progress(
            app,
            RequiredProgramInstallProgress {
                program_id: id.to_string(),
                phase: RequiredProgramInstallPhase::Downloading,
                bytes_received: received,
                bytes_total: total,
                message: None,
            },
        );
    })?;
    emit_install_progress(
        app,
        RequiredProgramInstallProgress {
            program_id: id.to_string(),
            phase: RequiredProgramInstallPhase::Extracting,
            bytes_received: bytes.len() as u64,
            bytes_total: Some(bytes.len() as u64),
            message: elevation_prompt_message(id, &filename),
        },
    );
    install_required_program_bytes(id, &filename, &bytes)?;
    let statuses = get_required_programs_status_blocking()?;
    let status = statuses
        .into_iter()
        .find(|s| s.definition.id == id)
        .ok_or_else(|| format!("Installed {id} but status was not found"))?;
    emit_install_progress(
        app,
        RequiredProgramInstallProgress {
            program_id: id.to_string(),
            phase: RequiredProgramInstallPhase::Done,
            bytes_received: bytes.len() as u64,
            bytes_total: Some(bytes.len() as u64),
            message: None,
        },
    );
    Ok(status)
}

fn emit_install_progress(app: &AppHandle, progress: RequiredProgramInstallProgress) {
    let _ = app.emit(INSTALL_PROGRESS_EVENT, &progress);
}

/// Place a portable payload under `data/programs/{id}` (USB-adjacent, never a Windows install).
///
/// Only a PE whose filename matches `exe_names`, or a zip that actually contains one of those
/// names, is kept. HTML landing pages and setup/installer EXEs are rejected.
pub fn install_required_program_bytes(
    id: &str,
    filename: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let def = REQUIRED_PROGRAMS
        .get(id)
        .ok_or_else(|| format!("Unknown required program: {id}"))?;
    let dest_dir = get_data_dir_path().join("programs").join(id);
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Failed to create programs directory: {e}"))?;

    if id == "smartctl" && is_smartmontools_setup(filename) && looks_like_pe(bytes) {
        // Official Windows build is an NSIS installer. Unpack only smartctl
        // into the programs folder; do not register it or change PATH.
        install_smartmontools_setup(&dest_dir, bytes)?;
    } else if looks_like_zip(bytes) {
        extract_zip_to_dir(bytes, &dest_dir)?;
    } else if looks_like_pe(bytes) {
        let dest_name = matching_exe_name(&def.exe_names, filename).ok_or_else(|| {
            format!(
                "No portable payload for {}. Expected a zip or {:?} — not '{}'.",
                def.name, def.exe_names, filename
            )
        })?;
        let dest = dest_dir.join(dest_name);
        fs::write(&dest, bytes).map_err(|e| format!("Failed to write program file: {e}"))?;
    } else {
        return Err(format!(
            "No portable payload for {}. Expected a zip or {:?} PE, got '{}'.",
            def.name, def.exe_names, filename
        ));
    }

    let placed = find_exe_in_programs_folder(&def.exe_names).ok_or_else(|| {
        format!(
            "Downloaded {} but none of {:?} were found under data/programs",
            def.name, def.exe_names
        )
    })?;
    let placed_bytes =
        fs::read(&placed).map_err(|e| format!("Failed to read placed program: {e}"))?;
    if !looks_like_pe(&placed_bytes) {
        let _ = fs::remove_dir_all(&dest_dir);
        return Err(format!(
            "Downloaded {} but {:?} is not a portable Windows executable",
            def.name, def.exe_names
        ));
    }
    Ok(placed)
}

fn matching_exe_name(exe_names: &[String], filename: &str) -> Option<String> {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    exe_names
        .iter()
        .find(|n| n.eq_ignore_ascii_case(base))
        .cloned()
}

fn looks_like_zip(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0] == 0x50 && bytes[1] == 0x4B
}

fn looks_like_pe(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == b'M' && bytes[1] == b'Z'
}

/// `https://github.com/{owner}/{repo}/releases/latest` → the releases API URL.
fn github_latest_release_api(url: &str) -> Option<String> {
    let rest = url
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if parts.next()? != "releases" || parts.next()? != "latest" || parts.next().is_some() {
        return None;
    }
    if owner.is_empty() || repo.is_empty() || owner.contains('.') || repo.contains('.') {
        return None;
    }
    Some(format!(
        "https://api.github.com/repos/{owner}/{repo}/releases/latest"
    ))
}

fn resolve_download_url(id: &str, url: &str) -> Result<String, String> {
    let Some(api) = github_latest_release_api(url) else {
        return Ok(url.to_string());
    };
    let body = http_get_text(&api)?;
    pick_latest_setup_url(id, &body)
}

fn http_get_text(url: &str) -> Result<String, String> {
    let response = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?
        .get(url)
        .header("User-Agent", "RustService")
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("Failed to read latest release: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Failed to read latest release: HTTP {}",
            response.status()
        ));
    }
    response
        .text()
        .map_err(|e| format!("Failed to read latest release: {e}"))
}

/// Pick the Windows installer asset from a GitHub release JSON document.
fn pick_latest_setup_url(id: &str, body: &str) -> Result<String, String> {
    let json: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| format!("Failed to read GitHub release: {e}"))?;
    let tag = json
        .get("tag_name")
        .and_then(|value| value.as_str())
        .unwrap_or("latest");
    let assets = json.get("assets").and_then(|value| value.as_array()).ok_or_else(|| {
        format!("GitHub release {tag} has no files")
    })?;
    assets
        .iter()
        .find_map(|asset| {
            let name = asset.get("name")?.as_str()?;
            if !release_asset_matches(id, name) {
                return None;
            }
            asset
                .get("browser_download_url")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .ok_or_else(|| format!("GitHub release {tag} has no Windows build for {id}"))
}

fn release_asset_matches(id: &str, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    match id {
        "smartctl" => name.ends_with(".win32-setup.exe"),
        _ => false,
    }
}

fn is_smartmontools_setup(filename: &str) -> bool {
    let name = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(filename)
        .to_ascii_lowercase();
    name.contains("smartmontools") && name.contains("setup") && name.ends_with(".exe")
}

/// Shown while the official installer is waiting on a UAC prompt.
fn elevation_prompt_message(id: &str, filename: &str) -> Option<String> {
    if id == "smartctl" && is_smartmontools_setup(filename) && !current_process_is_elevated() {
        Some("Waiting for administrator approval.".to_string())
    } else {
        None
    }
}

/// `runas` asks Windows for elevation. `open` is enough when this process is already elevated.
fn smartmontools_launch_verb(elevated: bool) -> &'static str {
    if elevated { "open" } else { "runas" }
}

/// NSIS reads `/D=` from the raw command line and rejects quotes around the path.
fn smartmontools_setup_parameters(dest: &str) -> String {
    format!("/S /SO smartctl,drivedb,x64 /D={dest}")
}

/// Run the official installer in extract-only mode into `dest_dir`.
///
/// `/SO smartctl,drivedb,x64` keeps the portable binaries and the drive
/// database. It leaves out the uninstaller, start menu, and PATH changes.
/// The installer requires administrator rights, so a non-elevated app asks
/// Windows to show the UAC prompt instead of failing with error 740.
fn install_smartmontools_setup(dest_dir: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create programs directory: {e}"))?;
    let setup_path = std::env::temp_dir().join("rustservice-smartmontools-setup.exe");
    fs::write(&setup_path, bytes).map_err(|e| format!("Failed to write installer: {e}"))?;
    let dest = std::path::absolute(dest_dir)
        .map_err(|e| format!("Failed to resolve install directory: {e}"))?;
    let dest = dest.to_string_lossy().trim_end_matches(['\\', '/']).to_string();

    let result = run_smartmontools_setup(&setup_path, &dest);
    let _ = fs::remove_file(&setup_path);
    result
}

#[cfg(windows)]
fn current_process_is_elevated() -> bool {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::securitybaseapi::GetTokenInformation;
    use winapi::um::winnt::{TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};

    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut TOKEN_ELEVATION as *mut _,
            u32::try_from(std::mem::size_of::<TOKEN_ELEVATION>()).unwrap_or(0),
            &mut size,
        );
        let _ = CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn current_process_is_elevated() -> bool {
    false
}

#[cfg(windows)]
fn wide_null(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// `CreateProcess` cannot start this installer when it needs elevation (error 740).
/// `ShellExecuteExW` lets Windows show the consent prompt.
#[cfg(windows)]
fn run_smartmontools_setup(setup_path: &Path, dest: &str) -> Result<(), String> {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetExitCodeProcess, TerminateProcess};
    use winapi::um::shellapi::{
        ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SEE_MASK_NO_CONSOLE,
        SHELLEXECUTEINFOW,
    };
    use winapi::um::synchapi::WaitForSingleObject;
    use winapi::um::winbase::{WAIT_FAILED, WAIT_OBJECT_0};
    use winapi::um::winnt::HANDLE;
    use winapi::um::winuser::SW_HIDE;

    const WAIT_TIMEOUT: u32 = 258;
    const ERROR_CANCELLED: i32 = 1223;

    struct ProcessHandle(HANDLE);
    impl Drop for ProcessHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    let verb = wide_null(smartmontools_launch_verb(current_process_is_elevated()));
    let file = wide_null(setup_path);
    let params = wide_null(smartmontools_setup_parameters(dest));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0);
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_NO_CONSOLE;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;

    let started = unsafe { ShellExecuteExW(&mut info) };
    if started == 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(ERROR_CANCELLED) {
            return Err(
                "Administrator approval was cancelled. Approve the prompt to install Smartctl."
                    .to_string(),
            );
        }
        return Err(format!("smartmontools installer failed to start: {err}"));
    }

    let handle = ProcessHandle(info.hProcess);
    if handle.0.is_null() {
        return Err("smartmontools installer failed to start.".to_string());
    }

    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let wait = unsafe { WaitForSingleObject(handle.0, 200) };
        if wait == WAIT_OBJECT_0 {
            break;
        }
        if wait == WAIT_FAILED {
            return Err(format!(
                "smartmontools installer failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        if wait != WAIT_TIMEOUT || Instant::now() >= deadline {
            unsafe {
                let _ = TerminateProcess(handle.0, 1);
            }
            return Err("smartmontools installer timed out after 180s.".to_string());
        }
    }

    let mut code = 0u32;
    let read = unsafe { GetExitCodeProcess(handle.0, &mut code) };
    if read == 0 {
        return Err(format!(
            "smartmontools installer failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    if code != 0 {
        return Err(format!("smartmontools installer exited with {code}."));
    }
    Ok(())
}

#[cfg(not(windows))]
fn run_smartmontools_setup(_setup_path: &Path, _dest: &str) -> Result<(), String> {
    Err("smartmontools install is only supported on Windows".to_string())
}

fn filename_from_response(response: &reqwest::blocking::Response, url: &str) -> String {
    if let Some(disp) = response.headers().get(reqwest::header::CONTENT_DISPOSITION) {
        if let Ok(s) = disp.to_str() {
            if let Some(name) = s.split("filename=").nth(1) {
                let cleaned = name.trim().trim_matches('"').trim_matches('\'');
                if !cleaned.is_empty() {
                    return cleaned
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(cleaned)
                        .to_string();
                }
            }
        }
    }
    url.split('?')
        .next()
        .unwrap_or(url)
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download.bin")
        .to_string()
}

fn extract_zip_to_dir(bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    let cursor = io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid zip archive: {e}"))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {e}"))?;
        if file.is_dir() {
            continue;
        }
        let enclosed = file
            .enclosed_name()
            .ok_or_else(|| format!("Refusing zip entry with unsafe path: {}", file.name()))?;
        let out_path = safe_join_under(dest_dir, &enclosed)?;
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create zip output dir: {e}"))?;
        }
        let mut out = fs::File::create(&out_path)
            .map_err(|e| format!("Failed to create {}: {e}", out_path.display()))?;
        io::copy(&mut file, &mut out).map_err(|e| format!("Failed to extract zip file: {e}"))?;
    }
    Ok(())
}

fn safe_join_under(dest_dir: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut out = dest_dir.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => {
                return Err(format!(
                    "Refusing zip entry with unsafe path: {}",
                    relative.display()
                ));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::data_dir::with_exclusive_data_dir;
    use std::io::{Cursor, Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::thread;

    fn with_temp_data_dir<R>(f: impl FnOnce() -> R) -> R {
        let dir = tempfile::tempdir().expect("temp data dir");
        with_exclusive_data_dir(dir.path().to_path_buf(), f)
    }

    fn serve_payload(filename: &'static str, body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture http");
        let addr = listener.local_addr().expect("local addr");
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"{filename}\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body);
            }
        });
        format!("http://{addr}/{filename}")
    }

    #[test]
    fn install_required_program_from_url_should_place_exe_and_mark_found() {
        with_temp_data_dir(|| {
            let payload = b"MZ-fixture-whynotwin11";
            let url = serve_payload("WhyNotWin11.exe", payload);
            let placed =
                install_required_program_from_url("whynotwin11", &url).expect("install fixture");
            assert!(placed.exists());
            assert!(placed.to_string_lossy().contains("programs"));
            assert_eq!(
                placed
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap()
                    .to_lowercase(),
                "whynotwin11.exe"
            );

            let statuses = get_required_programs_status_blocking().expect("status");
            let status = statuses
                .iter()
                .find(|s| s.definition.id == "whynotwin11")
                .expect("status row");
            assert!(status.found, "expected found after install: {status:?}");
            let path = status.path.as_deref().expect("path");
            assert!(path.to_lowercase().ends_with("whynotwin11.exe"));
        });
    }

    #[test]
    fn install_required_program_from_url_should_report_download_progress() {
        with_temp_data_dir(|| {
            let payload = b"MZ-fixture-whynotwin11-progress";
            let url = serve_payload("WhyNotWin11.exe", payload);
            let seen = Mutex::new(Vec::new());
            let placed = super::install_required_program_from_url_with_progress(
                "whynotwin11",
                &url,
                |received, total| {
                    seen.lock().expect("progress log").push((received, total));
                },
            )
            .expect("install fixture");
            assert!(placed.exists());
            let seen = seen.lock().expect("progress log");
            assert!(
                seen.iter().any(|(got, _)| *got == payload.len() as u64),
                "expected the full payload size in progress, got {seen:?}"
            );
            assert!(
                seen.iter()
                    .any(|(_, total)| *total == Some(payload.len() as u64)),
                "expected Content-Length in progress, got {seen:?}"
            );
        });
    }

    #[test]
    fn begin_install_should_reject_a_second_install_of_the_same_program() {
        let first = super::begin_install("test-lock-program").expect("first install");
        let err = super::begin_install("test-lock-program").expect_err("second install");
        assert!(err.contains("already installing"), "{err}");
        drop(first);
        let again = super::begin_install("test-lock-program").expect("after the first finished");
        drop(again);
    }

    #[test]
    fn install_required_program_from_url_should_error_on_http_failure() {
        with_temp_data_dir(|| {
            let err =
                install_required_program_from_url("whynotwin11", "http://127.0.0.1:1/missing.exe")
                    .expect_err("bad url");
            assert!(
                err.contains("Failed to download") || err.contains("error"),
                "{err}"
            );
        });
    }

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).expect("zip start_file");
            writer.write_all(data).expect("zip write");
        }
        writer.finish().expect("zip finish").into_inner()
    }

    #[test]
    fn install_required_program_bytes_should_reject_html_even_when_named_like_exe() {
        with_temp_data_dir(|| {
            let html = b"<!DOCTYPE html><html><body>download landing</body></html>";
            let err = install_required_program_bytes("furmark", "FurMark.exe", html)
                .expect_err("html is not portable");
            assert!(
                err.contains("portable") || err.contains("PE") || err.contains("Expected"),
                "{err}"
            );
            let dest = get_data_dir_path()
                .join("programs")
                .join("furmark")
                .join("FurMark.exe");
            assert!(!dest.exists(), "must not rename HTML to FurMark.exe");
        });
    }

    #[test]
    fn install_required_program_from_url_should_reject_setup_exe_not_matching_tool_name() {
        with_temp_data_dir(|| {
            let payload = b"MZ-setup-installer-not-the-tool";
            let url = serve_payload("HeavyLoad-x64-Setup.exe", payload);
            let err = install_required_program_from_url("heavyload", &url)
                .expect_err("setup exe is not portable");
            assert!(
                err.contains("portable") || err.contains("Expected") || err.contains("not"),
                "{err}"
            );
            let dest = get_data_dir_path()
                .join("programs")
                .join("heavyload")
                .join("HeavyLoad.exe");
            assert!(!dest.exists(), "must not rename setup.exe to HeavyLoad.exe");
        });
    }

    #[test]
    fn install_required_program_bytes_should_reject_zip_path_traversal() {
        with_temp_data_dir(|| {
            let zip = make_zip(&[("../../../evil.exe", b"MZ-traversal")]);
            let err = install_required_program_bytes("iperf3", "payload.zip", &zip)
                .expect_err("zip slip");
            assert!(err.contains("unsafe") || err.contains("Refusing"), "{err}");
        });
    }

    #[test]
    fn install_required_program_bytes_should_extract_zip_exe_and_mark_found() {
        with_temp_data_dir(|| {
            let zip = make_zip(&[("bin/iperf3.exe", b"MZ-portable-iperf")]);
            let placed =
                install_required_program_bytes("iperf3", "iperf.zip", &zip).expect("portable zip");
            assert!(placed.exists());
            assert!(placed
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap()
                .eq_ignore_ascii_case("iperf3.exe"));
            let statuses = get_required_programs_status_blocking().expect("status");
            let status = statuses
                .iter()
                .find(|s| s.definition.id == "iperf3")
                .expect("status row");
            assert!(status.found, "expected found after zip install: {status:?}");
        });
    }

    #[test]
    fn github_latest_release_api_should_map_the_releases_page_to_the_api() {
        assert_eq!(
            github_latest_release_api(
                "https://github.com/smartmontools/smartmontools/releases/latest"
            )
            .as_deref(),
            Some("https://api.github.com/repos/smartmontools/smartmontools/releases/latest")
        );
    }

    #[test]
    fn github_latest_release_api_should_ignore_a_pinned_asset_url() {
        assert_eq!(
            github_latest_release_api(
                "https://github.com/rcmaehl/WhyNotWin11/releases/latest/download/WhyNotWin11.exe"
            ),
            None
        );
    }

    #[test]
    fn resolve_download_url_should_leave_a_direct_file_url_unchanged() {
        let url = "https://github.com/rcmaehl/WhyNotWin11/releases/latest/download/WhyNotWin11.exe";
        let resolved = resolve_download_url("whynotwin11", url).expect("passthrough");
        assert_eq!(resolved, url);
    }

    #[test]
    fn pick_latest_setup_url_should_choose_the_windows_installer() {
        let body = r#"{
            "tag_name": "RELEASE_7_5",
            "assets": [
                {"name": "smartmontools-7.5.win32-setup.exe.asc", "browser_download_url": "https://example.invalid/setup.asc"},
                {"name": "smartmontools-7.5.win32-setup.exe.md5", "browser_download_url": "https://example.invalid/setup.md5"},
                {"name": "smartmontools-7.5.dmg", "browser_download_url": "https://example.invalid/mac.dmg"},
                {"name": "smartmontools-7.5.win32-setup.exe", "browser_download_url": "https://example.invalid/smartmontools-7.5.win32-setup.exe"}
            ]
        }"#;
        let url = pick_latest_setup_url("smartctl", body).expect("windows asset");
        assert_eq!(
            url,
            "https://example.invalid/smartmontools-7.5.win32-setup.exe"
        );
    }

    #[test]
    fn pick_latest_setup_url_should_error_when_the_release_has_no_windows_build() {
        let body = r#"{"tag_name":"RELEASE_7_5","assets":[{"name":"smartmontools-7.5.dmg","browser_download_url":"https://example.invalid/a.dmg"}]}"#;
        let err = pick_latest_setup_url("smartctl", body).expect_err("no windows build");
        assert!(err.contains("RELEASE_7_5"), "{err}");
    }

    #[test]
    fn is_smartmontools_setup_should_match_only_the_official_installer_name() {
        assert!(is_smartmontools_setup(
            "smartmontools-7.5.win32-setup.exe"
        ));
        assert!(!is_smartmontools_setup("HeavyLoad-x64-Setup.exe"));
        assert!(!is_smartmontools_setup(
            "smartmontools-7.5.win32-setup.exe.asc"
        ));
        assert!(!is_smartmontools_setup("setup.exe"));
    }

    #[test]
    fn smartmontools_launch_verb_should_request_elevation_only_when_needed() {
        assert_eq!(smartmontools_launch_verb(false), "runas");
        assert_eq!(smartmontools_launch_verb(true), "open");
    }

    #[test]
    fn smartmontools_setup_parameters_should_keep_the_install_dir_unquoted() {
        let params = smartmontools_setup_parameters(r"C:\Program Data\programs\smartctl");
        assert_eq!(
            params,
            r"/S /SO smartctl,drivedb,x64 /D=C:\Program Data\programs\smartctl"
        );
    }

    #[test]
    fn smartctl_download_url_should_point_at_the_latest_github_release() {
        let def = REQUIRED_PROGRAMS.get("smartctl").expect("smartctl");
        let url = def.download_url.as_deref().expect("download url");
        assert_eq!(
            url,
            "https://github.com/smartmontools/smartmontools/releases/latest"
        );
    }

    #[test]
    fn install_required_program_bytes_should_reject_a_smartmontools_named_file_for_another_program()
    {
        with_temp_data_dir(|| {
            let err = install_required_program_bytes(
                "heavyload",
                "smartmontools-7.5.win32-setup.exe",
                b"MZ-not-an-installer",
            )
            .expect_err("other program must not run the smartmontools installer");
            assert!(
                err.contains("portable") || err.contains("Expected"),
                "{err}"
            );
        });
    }
}
