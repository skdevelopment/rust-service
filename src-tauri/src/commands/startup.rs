//! Startup manager commands
//!
//! Commands for managing Windows startup programs from Registry and shell folders.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// Helpers
// ============================================================================

/// Escape a string for safe interpolation inside a PowerShell single-quoted string.
/// In PowerShell, a single quote inside single quotes is escaped by doubling it: ''
fn escape_powershell_single_quote(s: &str) -> String {
    s.replace('\'', "''")
}

// ============================================================================
// Types
// ============================================================================

/// Source location of a startup item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StartupSource {
    RegistryCurrentUser,
    RegistryLocalMachine,
    StartupFolderUser,
    StartupFolderAllUsers,
    TaskScheduler,
}

impl std::fmt::Display for StartupSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StartupSource::RegistryCurrentUser => write!(f, "Registry (Current User)"),
            StartupSource::RegistryLocalMachine => write!(f, "Registry (All Users)"),
            StartupSource::StartupFolderUser => write!(f, "Startup Folder (User)"),
            StartupSource::StartupFolderAllUsers => write!(f, "Startup Folder (All Users)"),
            StartupSource::TaskScheduler => write!(f, "Task Scheduler"),
        }
    }
}

/// Startup item information
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub command: String,
    pub path: Option<String>,
    pub source: StartupSource,
    pub source_location: String,
    pub enabled: bool,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub impact: StartupImpact,
}

/// Startup impact level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum StartupImpact {
    High,
    Medium,
    Low,
    Unknown,
}

// ============================================================================
// Commands
// ============================================================================

/// Get all startup items from various sources
#[tauri::command]
#[specta::specta]
pub async fn get_startup_items() -> Result<Vec<StartupItem>, String> {
    crate::proc::blocking(get_startup_items_blocking).await
}

fn get_startup_items_blocking() -> Result<Vec<StartupItem>, String> {
    let mut items = Vec::new();

    match get_registry_startup_items_sync() {
        Ok(mut registry_items) => items.append(&mut registry_items),
        Err(e) => log::warn!("Failed to get registry startup items: {}", e),
    }

    match get_startup_folder_items_sync() {
        Ok(mut folder_items) => items.append(&mut folder_items),
        Err(e) => log::warn!("Failed to get startup folder items: {}", e),
    }

    match get_scheduled_startup_tasks_sync() {
        Ok(mut task_items) => items.append(&mut task_items),
        Err(e) => log::warn!("Failed to get scheduled startup tasks: {}", e),
    }

    Ok(items)
}

/// Get startup items from registry
pub(crate) fn get_registry_startup_items_sync() -> Result<Vec<StartupItem>, String> {
    let output = crate::proc::output_with_timeout(crate::proc::command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"
            $items = @()
            
            # Current User Run
            $cuRun = Get-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue
            if ($cuRun) {
                $cuRun.PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' } | ForEach-Object {
                    $items += [PSCustomObject]@{
                        Name = $_.Name
                        Command = $_.Value
                        Source = 'RegistryCurrentUser'
                        SourceLocation = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
                        Enabled = $true
                    }
                }
            }
            
            # Current User RunOnce (enabled items)
            $cuRunOnce = Get-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' -ErrorAction SilentlyContinue
            if ($cuRunOnce) {
                $cuRunOnce.PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' } | ForEach-Object {
                    $items += [PSCustomObject]@{
                        Name = $_.Name
                        Command = $_.Value
                        Source = 'RegistryCurrentUser'
                        SourceLocation = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'
                        Enabled = $true
                    }
                }
            }
            
            # Local Machine Run
            $lmRun = Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' -ErrorAction SilentlyContinue
            if ($lmRun) {
                $lmRun.PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' } | ForEach-Object {
                    $items += [PSCustomObject]@{
                        Name = $_.Name
                        Command = $_.Value
                        Source = 'RegistryLocalMachine'
                        SourceLocation = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
                        Enabled = $true
                    }
                }
            }
            
            # Check for disabled items in Startup Approved
            $approvedCU = Get-ItemProperty -Path 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run' -ErrorAction SilentlyContinue
            $approvedLM = Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run' -ErrorAction SilentlyContinue
            
            # Update enabled status based on StartupApproved
            foreach ($item in $items) {
                $approved = if ($item.Source -eq 'RegistryCurrentUser') { $approvedCU } else { $approvedLM }
                if ($approved -and $approved.PSObject.Properties[$item.Name]) {
                    $bytes = $approved.($item.Name)
                    if ($bytes -and $bytes.Length -gt 0 -and $bytes[0] -ne 2) {
                        $item.Enabled = $false
                    }
                }
            }
            
            $items | ConvertTo-Json -Depth 3
            "#,
        ]), crate::proc::QUERY_TIMEOUT)
        .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

    let stdout = crate::proc::decode_output(&output.stdout);

    if stdout.trim().is_empty() || stdout.trim() == "null" {
        return Ok(Vec::new());
    }

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Failed to parse registry items: {} - Output: {}", e, stdout))?;

    let raw_items: Vec<&serde_json::Value> = if parsed.is_array() {
        parsed
            .as_array()
            .ok_or_else(|| "Expected JSON array from PowerShell registry output".to_string())?
            .iter()
            .collect()
    } else {
        vec![&parsed]
    };

    let mut items = Vec::new();
    for raw in raw_items {
        let name = raw["Name"].as_str().unwrap_or("Unknown").to_string();
        let command = raw["Command"].as_str().unwrap_or("").to_string();
        let source = match raw["Source"].as_str() {
            Some("RegistryCurrentUser") => StartupSource::RegistryCurrentUser,
            Some("RegistryLocalMachine") => StartupSource::RegistryLocalMachine,
            _ => StartupSource::RegistryCurrentUser,
        };
        let enabled = raw["Enabled"].as_bool().unwrap_or(true);

        let path = extract_path_from_command(&command);
        let impact = estimate_impact(&name, &command);

        let publisher = get_file_publisher(path.as_deref());
        let source_location = raw["SourceLocation"].as_str().unwrap_or("").to_string();
        items.push(StartupItem {
            id: encode_id("reg", &[&source_location, &name]),
            name,
            command,
            path,
            source,
            source_location,
            enabled,
            publisher,
            description: None,
            impact,
        });
    }

    Ok(items)
}

/// Get startup items from shell startup folders
pub(crate) fn get_startup_folder_items_sync() -> Result<Vec<StartupItem>, String> {
    let mut items = Vec::new();

    // User startup folder
    if let Some(user_folder) = dirs::data_local_dir() {
        let startup_path = user_folder
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup");

        items.extend(scan_startup_folder(
            &startup_path,
            StartupSource::StartupFolderUser,
        )?);
    }

    // All users startup folder
    let all_users_startup =
        PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup");
    items.extend(scan_startup_folder(
        &all_users_startup,
        StartupSource::StartupFolderAllUsers,
    )?);

    Ok(items)
}

/// Scan a startup folder for items
fn scan_startup_folder(path: &PathBuf, source: StartupSource) -> Result<Vec<StartupItem>, String> {
    let mut items = Vec::new();

    if !path.exists() {
        return Ok(items);
    }

    let entries =
        std::fs::read_dir(path).map_err(|e| format!("Failed to read startup folder: {}", e))?;

    for entry in entries.flatten() {
        let file_path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip desktop.ini and hidden files
        if file_name.starts_with('.') || file_name.to_lowercase() == "desktop.ini" {
            continue;
        }

        let name = file_name
            .trim_end_matches(".lnk")
            .trim_end_matches(".url")
            .to_string();
        let command = file_path.to_string_lossy().to_string();
        let impact = estimate_impact(&name, &command);
        let publisher = get_file_publisher(Some(&command));
        let source_location = path.to_string_lossy().to_string();

        items.push(StartupItem {
            id: encode_id("folder", &[&command]),
            name,
            path: Some(command.clone()),
            source: source.clone(),
            source_location,
            enabled: true, // Folder items are always "enabled" if they exist
            publisher,
            description: None,
            impact,
            command,
        });
    }

    Ok(items)
}

/// Get scheduled tasks with startup triggers
pub(crate) fn get_scheduled_startup_tasks_sync() -> Result<Vec<StartupItem>, String> {
    let output = crate::proc::output_with_timeout(crate::proc::command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"
            Get-ScheduledTask | Where-Object {
                $_.Triggers | Where-Object { $_.CimClass.CimClassName -eq 'MSFT_TaskBootTrigger' -or $_.CimClass.CimClassName -eq 'MSFT_TaskLogonTrigger' }
            } | ForEach-Object {
                $task = $_
                $info = Get-ScheduledTaskInfo -TaskName $_.TaskName -TaskPath $_.TaskPath -ErrorAction SilentlyContinue
                [PSCustomObject]@{
                    Name = $task.TaskName
                    Path = $task.TaskPath
                    State = $task.State.ToString()
                    Command = if ($task.Actions.Count -gt 0) { $task.Actions[0].Execute } else { '' }
                    Arguments = if ($task.Actions.Count -gt 0) { $task.Actions[0].Arguments } else { '' }
                    Description = $task.Description
                }
            } | ConvertTo-Json -Depth 3
            "#,
        ]), crate::proc::QUERY_TIMEOUT)
        .map_err(|e| format!("Failed to query scheduled tasks: {}", e))?;

    let stdout = crate::proc::decode_output(&output.stdout);

    if stdout.trim().is_empty() || stdout.trim() == "null" {
        return Ok(Vec::new());
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse tasks: {}", e))?;

    let raw_tasks: Vec<&serde_json::Value> = if parsed.is_array() {
        parsed
            .as_array()
            .ok_or_else(|| "Expected JSON array from PowerShell task output".to_string())?
            .iter()
            .collect()
    } else {
        vec![&parsed]
    };

    let mut items = Vec::new();
    for raw in raw_tasks {
        let name = raw["Name"].as_str().unwrap_or("Unknown").to_string();
        let command = raw["Command"].as_str().unwrap_or("").to_string();
        let args = raw["Arguments"].as_str().unwrap_or("");
        let full_command = if args.is_empty() {
            command.clone()
        } else {
            format!("{} {}", command, args)
        };
        let state = raw["State"].as_str().unwrap_or("Unknown");
        let enabled = state == "Ready" || state == "Running";
        let impact = estimate_impact(&name, &full_command);
        let source_location = raw["Path"].as_str().unwrap_or("").to_string();
        let description = raw["Description"].as_str().map(|s| s.to_string());

        items.push(StartupItem {
            id: encode_id("task", &[&source_location, &name]),
            name,
            command: full_command,
            path: Some(command),
            source: StartupSource::TaskScheduler,
            source_location,
            enabled,
            publisher: None,
            description,
            impact,
        });
    }

    Ok(items)
}

/// Toggle a startup item's enabled state
#[tauri::command]
#[specta::specta]
pub async fn toggle_startup_item(id: String, enabled: bool) -> Result<(), String> {
    crate::proc::blocking(move || toggle_startup_item_blocking(id, enabled)).await
}

fn toggle_startup_item_blocking(id: String, enabled: bool) -> Result<(), String> {
    set_startup_item_enabled(&id, enabled)
}

/// Enable or disable a startup item by id (shared with the startup optimizer).
pub(crate) fn set_startup_item_enabled(id: &str, enabled: bool) -> Result<(), String> {
    match decode_id(id)? {
        ItemRef::Registry { key, name } => set_registry_item_enabled(&key, &name, enabled),
        ItemRef::Task { path, name } => set_task_enabled(&path, &name, enabled),
        ItemRef::Folder { .. } => {
            Err("Startup folder items cannot be disabled - delete them instead".to_string())
        }
    }
}

/// The `Run` keys startup items are read from; ids must name one of these.
const RUN_KEYS: &[&str] = &[
    r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
    r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
    r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
    r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
];

/// The StartupApproved key Task Manager uses for a Run key's hive.
fn approved_key(run_key: &str) -> Result<String, String> {
    if !RUN_KEYS.contains(&run_key) {
        return Err(format!("Unsupported startup registry key: {}", run_key));
    }
    if run_key.ends_with("RunOnce") {
        return Err("RunOnce entries run once at next logon and cannot be disabled".to_string());
    }
    let hive = &run_key[..4];
    Ok(format!(
        r"{}:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        hive
    ))
}

fn run_powershell(script: &str, what: &str) -> Result<(), String> {
    let script = format!("$ErrorActionPreference = 'Stop'\n{}", script);
    let output = crate::proc::output_with_timeout(
        crate::proc::command("powershell").args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]),
        crate::proc::QUERY_TIMEOUT,
    )
    .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

    if !output.status.success() {
        let stderr = crate::proc::decode_output(&output.stderr);
        return Err(format!("Failed to {}: {}", what, stderr.trim()));
    }
    Ok(())
}

/// Enable/disable a Run-key item the way Task Manager does: via a
/// StartupApproved value (created if missing; 02 = enabled, 03 = disabled).
fn set_registry_item_enabled(run_key: &str, name: &str, enabled: bool) -> Result<(), String> {
    let approved = approved_key(run_key)?;
    let flag = if enabled { 2 } else { 3 };
    run_powershell(
        &format!(
            r#"
            $key = '{key}'
            if (-not (Test-Path $key)) {{ New-Item -Path $key -Force | Out-Null }}
            $bytes = [byte[]]@({flag},0,0,0,0,0,0,0,0,0,0,0)
            New-ItemProperty -Path $key -Name '{name}' -PropertyType Binary -Value $bytes -Force | Out-Null
            "#,
            key = escape_powershell_single_quote(&approved),
            name = escape_powershell_single_quote(name),
            flag = flag
        ),
        if enabled {
            "enable startup item"
        } else {
            "disable startup item"
        },
    )
}

fn set_task_enabled(task_path: &str, name: &str, enabled: bool) -> Result<(), String> {
    let action = if enabled { "Enable" } else { "Disable" };
    run_powershell(
        &format!(
            "{}-ScheduledTask -TaskPath '{}' -TaskName '{}' | Out-Null",
            action,
            escape_powershell_single_quote(task_path),
            escape_powershell_single_quote(name)
        ),
        &format!("{} scheduled task", action.to_lowercase()),
    )
}

/// Open the file location of a startup item in Explorer
#[tauri::command]
#[specta::specta]
pub async fn open_startup_item_location(path: String) -> Result<(), String> {
    crate::proc::blocking(move || open_startup_item_location_blocking(path)).await
}

fn open_startup_item_location_blocking(path: String) -> Result<(), String> {
    let file_path = std::path::Path::new(&path);

    if file_path.exists() {
        // Use explorer /select to highlight the file
        crate::proc::command("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    } else if let Some(parent) = file_path.parent() {
        if parent.exists() {
            crate::proc::command("explorer")
                .arg(parent.to_string_lossy().as_ref())
                .spawn()
                .map_err(|e| format!("Failed to open Explorer: {}", e))?;
        } else {
            return Err(format!("Path does not exist: {}", path));
        }
    } else {
        return Err(format!("Path does not exist: {}", path));
    }

    Ok(())
}

/// Delete a startup item
#[tauri::command]
#[specta::specta]
pub async fn delete_startup_item(id: String) -> Result<(), String> {
    crate::proc::blocking(move || delete_startup_item_blocking(id)).await
}

fn delete_startup_item_blocking(id: String) -> Result<(), String> {
    match decode_id(&id)? {
        ItemRef::Registry { key, name } => delete_registry_startup_item(&key, &name),
        ItemRef::Folder { path } => delete_startup_folder_item(&path),
        ItemRef::Task { path, name } => delete_scheduled_task(&path, &name),
    }
}

/// Delete a registry startup item (and its StartupApproved state)
fn delete_registry_startup_item(run_key: &str, name: &str) -> Result<(), String> {
    if !RUN_KEYS.contains(&run_key) {
        return Err(format!("Unsupported startup registry key: {}", run_key));
    }
    let approved = format!(
        r"{}:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        &run_key[..4]
    );
    run_powershell(
        &format!(
            r#"
            Remove-ItemProperty -Path '{key}' -Name '{name}'
            Remove-ItemProperty -Path '{approved}' -Name '{name}' -ErrorAction SilentlyContinue
            "#,
            key = escape_powershell_single_quote(run_key),
            approved = escape_powershell_single_quote(&approved),
            name = escape_powershell_single_quote(name)
        ),
        "delete startup item",
    )
}

/// Delete a startup folder item
fn delete_startup_folder_item(file_path: &str) -> Result<(), String> {
    let path = std::path::Path::new(file_path);

    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path));
    }

    // Safety: ensure the file is actually inside a known startup folder
    let allowed_dirs: Vec<PathBuf> = {
        let mut allowed = Vec::new();
        if let Some(local) = dirs::data_local_dir() {
            allowed.push(
                local
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs")
                    .join("Startup"),
            );
        }
        allowed.push(PathBuf::from(
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup",
        ));
        allowed
    };

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;

    let is_in_startup_folder = allowed_dirs.iter().any(|dir| {
        if let Ok(canon_dir) = dir.canonicalize() {
            canonical.starts_with(&canon_dir)
        } else {
            false
        }
    });

    if !is_in_startup_folder {
        return Err(format!(
            "Refusing to delete file outside startup folders: {}",
            file_path
        ));
    }

    std::fs::remove_file(path).map_err(|e| format!("Failed to delete startup item: {}", e))?;

    Ok(())
}

/// Delete a scheduled task
fn delete_scheduled_task(task_path: &str, name: &str) -> Result<(), String> {
    run_powershell(
        &format!(
            "Unregister-ScheduledTask -TaskPath '{}' -TaskName '{}' -Confirm:$false",
            escape_powershell_single_quote(task_path),
            escape_powershell_single_quote(name)
        ),
        "delete scheduled task",
    )
}

// ============================================================================
// Helper Functions
// ============================================================================

/// What a startup item id refers to.
#[derive(Debug, PartialEq)]
enum ItemRef {
    Registry { key: String, name: String },
    Folder { path: String },
    Task { path: String, name: String },
}

const ID_SEPARATOR: char = '\u{1f}';

/// Build an id that encodes everything needed to act on the item later.
/// Hex keeps it safe for DOM keys while staying lossless.
fn encode_id(kind: &str, parts: &[&str]) -> String {
    let joined = parts.join(&ID_SEPARATOR.to_string());
    format!("{}_{}", kind, hex::encode(joined.as_bytes()))
}

fn decode_id(id: &str) -> Result<ItemRef, String> {
    let invalid = || format!("Invalid startup item id: {}", id);
    let (kind, payload) = id.split_once('_').ok_or_else(invalid)?;
    let bytes = hex::decode(payload).map_err(|_| invalid())?;
    let text = String::from_utf8(bytes).map_err(|_| invalid())?;
    let mut parts = text.split(ID_SEPARATOR).map(str::to_owned);
    let mut next = || parts.next().ok_or_else(invalid);
    match kind {
        "reg" => Ok(ItemRef::Registry {
            key: next()?,
            name: next()?,
        }),
        "folder" => Ok(ItemRef::Folder { path: next()? }),
        "task" => Ok(ItemRef::Task {
            path: next()?,
            name: next()?,
        }),
        _ => Err(format!("Unknown startup item type: {}", id)),
    }
}

/// Extract executable path from a command string
fn extract_path_from_command(command: &str) -> Option<String> {
    let cmd = command.trim();

    // Handle quoted paths
    if let Some(rest) = cmd.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }

    // Handle unquoted paths - take until first space or whole string
    let path = cmd.split_whitespace().next()?;
    Some(path.to_string())
}

/// Estimate startup impact based on known programs
fn estimate_impact(name: &str, _command: &str) -> StartupImpact {
    let name_lower = name.to_lowercase();

    // High impact - known resource-heavy programs
    let high_impact = [
        "steam", "discord", "spotify", "adobe", "office", "teams", "slack", "zoom",
    ];
    if high_impact.iter().any(|&p| name_lower.contains(p)) {
        return StartupImpact::High;
    }

    // Low impact - system utilities
    let low_impact = [
        "defender", "security", "audio", "realtek", "nvidia", "amd", "intel", "logitech",
    ];
    if low_impact.iter().any(|&p| name_lower.contains(p)) {
        return StartupImpact::Low;
    }

    // Medium for everything else known
    StartupImpact::Medium
}

/// Get publisher (CompanyName) from file version info
fn get_file_publisher(exe_path: Option<&str>) -> Option<String> {
    let path = exe_path?;
    if path.is_empty() {
        return None;
    }

    // Only attempt for .exe / .dll files that exist
    let p = std::path::Path::new(path);
    if !p.exists() {
        return None;
    }

    #[cfg(windows)]
    {
        use widestring::U16CString;

        let wide_path = U16CString::from_str(path).ok()?;
        let mut handle: u32 = 0;

        let size =
            unsafe { winapi::um::winver::GetFileVersionInfoSizeW(wide_path.as_ptr(), &mut handle) };
        if size == 0 {
            return None;
        }

        let mut buffer: Vec<u8> = vec![0u8; size as usize];
        let success = unsafe {
            winapi::um::winver::GetFileVersionInfoW(
                wide_path.as_ptr(),
                handle,
                size,
                buffer.as_mut_ptr() as *mut _,
            )
        };
        if success == 0 {
            return None;
        }

        // Try common language/codepage combos for CompanyName
        let sub_blocks = [
            "\\StringFileInfo\\040904B0\\CompanyName", // English US, Unicode
            "\\StringFileInfo\\040904E4\\CompanyName", // English US, Windows Latin-1
            "\\StringFileInfo\\000004B0\\CompanyName", // Language neutral, Unicode
        ];

        for sub_block in &sub_blocks {
            let wide_sub = U16CString::from_str(*sub_block).ok()?;
            let mut lp_buffer: *mut winapi::ctypes::c_void = std::ptr::null_mut();
            let mut len: u32 = 0;

            let ok = unsafe {
                winapi::um::winver::VerQueryValueW(
                    buffer.as_ptr() as *const _,
                    wide_sub.as_ptr(),
                    &mut lp_buffer,
                    &mut len,
                )
            };

            if ok != 0 && len > 0 && !lp_buffer.is_null() {
                let slice =
                    unsafe { std::slice::from_raw_parts(lp_buffer as *const u16, len as usize) };
                let company = String::from_utf16_lossy(slice)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                if !company.is_empty() {
                    return Some(company);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_names_with_spaces_and_dots() {
        let key = RUN_KEYS[0];
        let id = encode_id("reg", &[key, "Microsoft Teams (work or school).exe"]);
        assert_eq!(
            decode_id(&id).unwrap(),
            ItemRef::Registry {
                key: key.to_string(),
                name: "Microsoft Teams (work or school).exe".into()
            }
        );
        let id = encode_id("task", &[r"\Vendor\", "Updater Task"]);
        assert_eq!(
            decode_id(&id).unwrap(),
            ItemRef::Task {
                path: r"\Vendor\".into(),
                name: "Updater Task".into()
            }
        );
        assert!(decode_id("reg_zz").is_err());
    }

    #[test]
    fn approved_key_rejects_unknown_and_runonce_keys() {
        assert!(approved_key(r"HKLM:\SOFTWARE\Evil").is_err());
        assert!(approved_key(RUN_KEYS[1]).is_err());
        assert_eq!(
            approved_key(RUN_KEYS[2]).unwrap(),
            r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
        );
    }
}
