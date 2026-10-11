//! Required programs commands
//!
//! Tauri commands for managing required external programs.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use super::data_dir::get_data_dir_path;
use super::settings::get_settings;
use crate::types::{RequiredProgramDef, RequiredProgramStatus};

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
            url: Some("https://www.smartmontools.org/wiki/Download".to_string()),
            download_url: None,
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

pub(crate) fn get_required_programs_status_blocking() -> Result<Vec<RequiredProgramStatus>, String> {
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
#[tauri::command]
#[specta::specta]
pub async fn install_required_program(id: String) -> Result<RequiredProgramStatus, String> {
    crate::proc::blocking(move || install_required_program_blocking(&id)).await
}

/// Download `url` and install it as required program `id` (testable unit).
pub fn install_required_program_from_url(id: &str, url: &str) -> Result<PathBuf, String> {
    let def = REQUIRED_PROGRAMS
        .get(id)
        .ok_or_else(|| format!("Unknown required program: {id}"))?;
    if url.trim().is_empty() {
        return Err(format!(
            "No unattended download is available for {}",
            def.name
        ));
    }

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
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read download body: {e}"))?;
    install_required_program_bytes(id, &filename, &bytes)
}

fn install_required_program_blocking(id: &str) -> Result<RequiredProgramStatus, String> {
    let def = REQUIRED_PROGRAMS
        .get(id)
        .ok_or_else(|| format!("Unknown required program: {id}"))?;
    let url = def.download_url.clone().unwrap_or_default();
    install_required_program_from_url(id, &url)?;
    let statuses = get_required_programs_status_blocking()?;
    statuses
        .into_iter()
        .find(|s| s.definition.id == id)
        .ok_or_else(|| format!("Installed {id} but status was not found"))
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

    if looks_like_zip(bytes) {
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
    let base = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(filename);
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

fn filename_from_response(response: &reqwest::blocking::Response, url: &str) -> String {
    if let Some(disp) = response.headers().get(reqwest::header::CONTENT_DISPOSITION) {
        if let Ok(s) = disp.to_str() {
            if let Some(name) = s.split("filename=").nth(1) {
                let cleaned = name.trim().trim_matches('"').trim_matches('\'');
                if !cleaned.is_empty() {
                    return cleaned.rsplit(['/', '\\']).next().unwrap_or(cleaned).to_string();
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
        let enclosed = file.enclosed_name().ok_or_else(|| {
            format!("Refusing zip entry with unsafe path: {}", file.name())
        })?;
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
            let placed = install_required_program_from_url("whynotwin11", &url)
                .expect("install fixture");
            assert!(placed.exists());
            assert!(placed.to_string_lossy().contains("programs"));
            assert_eq!(
                placed.file_name().and_then(|n| n.to_str()).unwrap().to_lowercase(),
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
    fn install_required_program_from_url_should_error_on_http_failure() {
        with_temp_data_dir(|| {
            let err = install_required_program_from_url(
                "whynotwin11",
                "http://127.0.0.1:1/missing.exe",
            )
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
            assert!(
                err.contains("unsafe") || err.contains("Refusing"),
                "{err}"
            );
        });
    }

    #[test]
    fn install_required_program_bytes_should_extract_zip_exe_and_mark_found() {
        with_temp_data_dir(|| {
            let zip = make_zip(&[("bin/iperf3.exe", b"MZ-portable-iperf")]);
            let placed = install_required_program_bytes("iperf3", "iperf.zip", &zip)
                .expect("portable zip");
            assert!(placed.exists());
            assert!(
                placed
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap()
                    .eq_ignore_ascii_case("iperf3.exe")
            );
            let statuses = get_required_programs_status_blocking().expect("status");
            let status = statuses
                .iter()
                .find(|s| s.definition.id == "iperf3")
                .expect("status row");
            assert!(status.found, "expected found after zip install: {status:?}");
        });
    }
}
