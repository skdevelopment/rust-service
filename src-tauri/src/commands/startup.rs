//! Startup manager commands
//!
//! Commands for managing Windows startup programs from Registry and shell folders.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    for (dir, source, approved) in startup_folders() {
        let approved = read_startup_approved_map(approved);
        items.extend(scan_startup_folder(&dir, source, &approved)?);
    }
    Ok(items)
}

/// Scan a startup folder for items
fn scan_startup_folder(
    path: &PathBuf,
    source: StartupSource,
    approved: &HashMap<String, bool>,
) -> Result<Vec<StartupItem>, String> {
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
        let enabled = approved_enabled_for(&file_name, approved);

        items.push(StartupItem {
            id: encode_id("folder", &[&command]),
            name,
            path: Some(command.clone()),
            source: source.clone(),
            source_location,
            enabled,
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
        ItemRef::Folder { path } => set_folder_item_enabled(&path, enabled),
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
    set_approved_value(&approved_key(run_key)?, name, enabled)
}

/// Enable/disable a startup-folder item through StartupApproved\StartupFolder,
/// keyed by its file name (what Task Manager does; the file stays in place).
fn set_folder_item_enabled(file_path: &str, enabled: bool) -> Result<(), String> {
    let (file, approved) = locate_folder_item(file_path)?;
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Startup folder item has no file name".to_string())?;
    set_approved_value(approved, name, enabled)
}

fn set_approved_value(key: &str, name: &str, enabled: bool) -> Result<(), String> {
    let flag = if enabled { 2 } else { 3 };
    run_powershell(
        &format!(
            r#"
            $key = '{key}'
            if (-not (Test-Path -LiteralPath $key)) {{ New-Item -Path $key -Force | Out-Null }}
            $bytes = [byte[]]@({flag},0,0,0,0,0,0,0,0,0,0,0)
            New-ItemProperty -LiteralPath $key -Name '{name}' -PropertyType Binary -Value $bytes -Force | Out-Null
            "#,
            key = escape_powershell_single_quote(key),
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

const USER_FOLDER_APPROVED: &str =
    r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";
const MACHINE_FOLDER_APPROVED: &str =
    r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder";

/// The per-user and all-users startup folders, with the StartupApproved key
/// that records each one's enabled state.
fn startup_folders() -> Vec<(PathBuf, StartupSource, &'static str)> {
    let startup = Path::new("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup");
    let mut folders = Vec::new();
    // The per-user folder lives in Roaming AppData (%APPDATA%), not Local.
    if let Some(roaming) = dirs::data_dir() {
        folders.push((
            roaming.join(&startup),
            StartupSource::StartupFolderUser,
            USER_FOLDER_APPROVED,
        ));
    }
    let program_data = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    folders.push((
        program_data.join(&startup),
        StartupSource::StartupFolderAllUsers,
        MACHINE_FOLDER_APPROVED,
    ));
    folders
}

/// Resolve a startup-folder item to its file and StartupApproved key, refusing
/// anything that is not directly inside one of the startup folders.
fn locate_folder_item(file_path: &str) -> Result<(PathBuf, &'static str), String> {
    let canonical = Path::new(file_path)
        .canonicalize()
        .map_err(|e| format!("Startup item not found ({}): {}", file_path, e))?;
    for (dir, _, approved) in startup_folders() {
        if let Ok(dir) = dir.canonicalize() {
            if canonical.parent() == Some(dir.as_path()) {
                return Ok((canonical, approved));
            }
        }
    }
    Err(format!(
        "Refusing to touch a file outside the startup folders: {}",
        file_path
    ))
}

/// Read a StartupApproved key into `value name -> enabled` (missing key: empty).
fn read_startup_approved_map(key: &str) -> HashMap<String, bool> {
    let script = format!(
        r#"
        $item = Get-ItemProperty -LiteralPath '{key}' -ErrorAction SilentlyContinue
        $rows = @()
        if ($item) {{
            foreach ($prop in $item.PSObject.Properties) {{
                if ($prop.Name -like 'PS*') {{ continue }}
                $bytes = [byte[]]$prop.Value
                $enabled = -not ($bytes -and $bytes.Length -gt 0 -and $bytes[0] -ne 2)
                $rows += [PSCustomObject]@{{ Name = $prop.Name; Enabled = [bool]$enabled }}
            }}
        }}
        ConvertTo-Json -InputObject @($rows) -Compress -Depth 2
        "#,
        key = escape_powershell_single_quote(key)
    );
    let Ok(output) = crate::proc::output_with_timeout(
        crate::proc::command("powershell").args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]),
        crate::proc::QUERY_TIMEOUT,
    ) else {
        return HashMap::new();
    };
    let stdout = crate::proc::decode_output(&output.stdout);
    let rows: Vec<serde_json::Value> = serde_json::from_str(stdout.trim()).unwrap_or_default();
    rows.iter()
        .filter_map(|row| {
            let name = row.get("Name")?.as_str()?.to_string();
            let enabled = row.get("Enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            Some((name, enabled))
        })
        .collect()
}

/// Task Manager treats a missing StartupApproved value as enabled.
fn approved_enabled_for(name: &str, map: &HashMap<String, bool>) -> bool {
    map.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, enabled)| *enabled)
        .unwrap_or(true)
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

/// Delete a startup folder item (and its StartupApproved state)
fn delete_startup_folder_item(file_path: &str) -> Result<(), String> {
    let (file, approved) = locate_folder_item(file_path)?;
    std::fs::remove_file(&file).map_err(|e| format!("Failed to delete startup item: {}", e))?;
    if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
        let _ = run_powershell(
            &format!(
                "Remove-ItemProperty -LiteralPath '{}' -Name '{}' -ErrorAction SilentlyContinue",
                escape_powershell_single_quote(approved),
                escape_powershell_single_quote(name)
            ),
            "clear startup item state",
        );
    }
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

    use std::fs;
    use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn unique_name() -> String {
        format!("RustServiceStartupTest{}", uuid::Uuid::new_v4().simple())
    }

    fn run_ps(script: &str) {
        let _ = run_powershell(script, "run test setup");
    }

    #[test]
    fn toggle_startup_item_should_disable_hkcu_run_and_read_back_disabled() {
        let _guard = crate::lock::lock_mutex(&TEST_LOCK);
        let name = unique_name();
        let safe = escape_powershell_single_quote(&name);
        run_ps(&format!(
            "New-ItemProperty -LiteralPath '{}' -Name '{safe}' -PropertyType String -Value 'rundll32.exe' -Force | Out-Null",
            RUN_KEYS[0]
        ));
        let id = encode_id("reg", &[RUN_KEYS[0], &name]);
        let result = catch_unwind(AssertUnwindSafe(|| {
            set_startup_item_enabled(&id, false).expect("toggle off");
            let items = get_registry_startup_items_sync().expect("list registry startup");
            let item = items
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(&name))
                .unwrap_or_else(|| panic!("test Run value {name} not listed: {items:?}"));
            assert_eq!(item.id, id);
            assert!(!item.enabled, "expected enabled=false after toggle off, got {item:?}");
        }));
        run_ps(&format!(
            "Remove-ItemProperty -LiteralPath '{}' -Name '{safe}' -ErrorAction SilentlyContinue\nRemove-ItemProperty -LiteralPath '{}' -Name '{safe}' -ErrorAction SilentlyContinue",
            RUN_KEYS[0],
            approved_key(RUN_KEYS[0]).unwrap()
        ));
        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }

    #[test]
    fn toggle_startup_item_should_disable_startup_folder_item_and_read_back_disabled() {
        let _guard = crate::lock::lock_mutex(&TEST_LOCK);
        let name = unique_name();
        let file_name = format!("{name}.lnk");
        let (dir, _, approved) = startup_folders()
            .into_iter()
            .find(|(_, source, _)| *source == StartupSource::StartupFolderUser)
            .expect("user startup folder");
        fs::create_dir_all(&dir).expect("create user startup folder");
        let path = dir.join(&file_name);
        fs::write(&path, b"").expect("write dummy startup file");
        let result = catch_unwind(AssertUnwindSafe(|| {
            let find = || {
                let items = get_startup_folder_items_sync().expect("list folder startup");
                items
                    .into_iter()
                    .find(|i| i.name.eq_ignore_ascii_case(&name))
                    .unwrap_or_else(|| panic!("test folder item {name} not listed"))
            };
            let item = find();
            assert!(item.enabled, "new folder items start enabled: {item:?}");
            set_startup_item_enabled(&item.id, false).expect("toggle folder off");
            let item = find();
            assert!(!item.enabled, "expected enabled=false after toggle off, got {item:?}");
        }));
        let _ = fs::remove_file(&path);
        run_ps(&format!(
            "Remove-ItemProperty -LiteralPath '{approved}' -Name '{}' -ErrorAction SilentlyContinue",
            escape_powershell_single_quote(&file_name)
        ));
        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }
}
