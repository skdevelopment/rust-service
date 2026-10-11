//! Settings management commands
//!
//! `data/settings.json` is shared by the UI, the service runner, the agent and
//! the MCP server, so every access goes through this module:
//!
//! - reads never write (a migrated file is only written on the next real change),
//! - read-modify-write cycles are serialized by one lock ([`update_settings`]),
//! - writes are atomic (temp file + rename), so a crash never leaves half a file,
//! - a file that fails to parse is backed up and parsed section by section,
//!   instead of being silently replaced with defaults.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use crate::commands::data_dir::get_data_dir_path;
use crate::types::{migrate_settings, AppSettings};

/// Serializes every settings read-modify-write cycle in the process.
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

fn settings_path() -> PathBuf {
    get_data_dir_path().join("settings.json")
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    // The guarded data is the file itself; a panic elsewhere cannot leave it
    // half-written (writes are atomic), so a poisoned lock is safe to reuse.
    SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Load settings without taking the lock. Callers must hold [`lock`].
fn load_unlocked() -> Result<AppSettings, String> {
    let path = settings_path();
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppSettings::default());
        }
        Err(e) => return Err(format!("Failed to read settings: {}", e)),
    };

    let settings = match serde_json::from_str::<AppSettings>(&content) {
        Ok(s) => s,
        Err(err) => recover_settings(&path, &content, &err.to_string()),
    };

    Ok(migrate_settings(settings))
}

/// Build the best settings we can from a file that failed strict parsing.
///
/// The original file is copied aside first so nothing is ever lost.
fn recover_settings(path: &Path, content: &str, error: &str) -> AppSettings {
    let backup = path.with_file_name(format!(
        "settings.corrupt-{}.json",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    match fs::copy(path, &backup) {
        Ok(_) => log::warn!(
            "settings.json failed to parse ({}); backed up to {}",
            error,
            backup.display()
        ),
        Err(e) => log::error!(
            "settings.json failed to parse ({}); backup failed: {}",
            error,
            e
        ),
    }

    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(content) else {
        log::error!("settings.json is not a JSON object; using defaults");
        return AppSettings::default();
    };

    // Start from defaults and keep every section (or, failing that, every
    // field of a section) that still parses.
    let mut merged =
        serde_json::to_value(AppSettings::default()).expect("default settings always serialize");
    let merged_obj = merged
        .as_object_mut()
        .expect("settings serialize to an object");
    let accepts = |candidate: &serde_json::Map<String, Value>| {
        serde_json::from_value::<AppSettings>(Value::Object(candidate.clone())).is_ok()
    };
    for (key, section) in root {
        let mut candidate = merged_obj.clone();
        candidate.insert(key.clone(), section.clone());
        if accepts(&candidate) {
            *merged_obj = candidate;
            continue;
        }
        let Value::Object(fields) = section else {
            log::warn!("Dropping unreadable settings section '{}'", key);
            continue;
        };
        for (field, field_value) in fields {
            let mut candidate = merged_obj.clone();
            if let Some(Value::Object(target)) = candidate.get_mut(&key) {
                target.insert(field.clone(), field_value);
            }
            if accepts(&candidate) {
                *merged_obj = candidate;
            } else {
                log::warn!("Dropping unreadable setting '{}.{}'", key, field);
            }
        }
    }

    // Pre-0.5 files kept the theme at the root.
    if let Some(theme) = merged_obj
        .get("theme")
        .and_then(Value::as_str)
        .map(str::to_owned)
    {
        if let Some(Value::Object(appearance)) = merged_obj.get_mut("appearance") {
            if matches!(theme.as_str(), "light" | "dark" | "system") {
                appearance.insert("theme".into(), Value::String(theme));
            }
        }
    }

    serde_json::from_value(merged).unwrap_or_default()
}

/// Atomically replace settings.json. Callers must hold [`lock`].
fn write_unlocked(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create data directory: {}", e))?;
    }

    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("Failed to write settings: {}", e))?;

    // Antivirus or indexers can briefly hold the target open; retry a few times.
    let mut last_err = None;
    for _ in 0..5 {
        match fs::rename(&tmp, &path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
    let _ = fs::remove_file(&tmp);
    Err(format!(
        "Failed to replace settings file: {}",
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// Read the current settings (never writes).
pub fn load_settings() -> Result<AppSettings, String> {
    let _guard = lock();
    load_unlocked()
}

/// Apply `change` to the current settings and persist the result atomically.
///
/// The whole read-modify-write runs under one lock, so concurrent writers
/// (UI, service runner, agent) cannot overwrite each other's changes.
pub fn update_settings<T>(
    change: impl FnOnce(&mut AppSettings) -> Result<T, String>,
) -> Result<(AppSettings, T), String> {
    let _guard = lock();
    let mut settings = load_unlocked()?;
    let out = change(&mut settings)?;
    write_unlocked(&settings)?;
    Ok((settings, out))
}

/// Retrieves the current application settings
///
/// Returns defaults if the file doesn't exist yet.
#[tauri::command(async)]
#[specta::specta]
pub fn get_settings() -> Result<AppSettings, String> {
    load_settings()
}

/// Updates a single setting by its dotted camelCase path
///
/// # Arguments
/// * `key` - Path to the setting (e.g. "appearance.theme", "agent.apiKeys.openai", or "agent")
/// * `value` - JSON-encoded value to store at that path
#[tauri::command(async)]
#[specta::specta]
pub fn update_setting(key: String, value: String) -> Result<AppSettings, String> {
    let new_value: Value =
        serde_json::from_str(&value).map_err(|e| format!("Invalid JSON for {}: {}", key, e))?;

    let (settings, ()) = update_settings(|settings| {
        *settings = set_setting_path(settings, &key, new_value)?;
        Ok(())
    })?;

    apply_runtime_settings(&settings);
    Ok(settings)
}

/// Push settings that affect running subsystems (log level, MCP server).
fn apply_runtime_settings(settings: &AppSettings) {
    crate::logging::apply_settings(&settings.data);
    crate::mcp::apply_settings(&settings.agent);
}

/// Return a copy of `settings` with the value at dotted `key` replaced.
///
/// The result is round-tripped through `AppSettings`, so values of the wrong
/// type are rejected and unknown keys are reported instead of silently dropped.
fn set_setting_path(
    settings: &AppSettings,
    key: &str,
    value: Value,
) -> Result<AppSettings, String> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("Invalid setting key: {}", key));
    }

    let mut tree = serde_json::to_value(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    let (leaf, parents) = parts
        .split_last()
        .expect("split on '.' yields at least one part");
    let mut node = &mut tree;
    for part in parents {
        let obj = node
            .as_object_mut()
            .ok_or_else(|| format!("Unknown setting key: {}", key))?;
        node = obj
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }
    node.as_object_mut()
        .ok_or_else(|| format!("Unknown setting key: {}", key))?
        .insert(leaf.to_string(), value.clone());

    let updated: AppSettings =
        serde_json::from_value(tree).map_err(|e| format!("Invalid value for {}: {}", key, e))?;

    // serde ignores unknown fields; make sure the value actually landed.
    if !value.is_null() {
        let check = serde_json::to_value(&updated)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        let pointer = format!("/{}", parts.join("/"));
        if check.pointer(&pointer).is_none() {
            return Err(format!("Unknown setting key: {}", key));
        }
    }

    Ok(updated)
}

/// Get the business logo directory path
fn get_business_dir() -> PathBuf {
    get_data_dir_path().join("business")
}

/// Save business logo by copying to data directory
/// Returns the relative path to the saved logo
#[tauri::command(async)]
#[specta::specta]
pub fn save_business_logo(source_path: String) -> Result<String, String> {
    let source = Path::new(&source_path);

    if !source.is_file() {
        return Err("Source file does not exist".to_string());
    }

    // Only accept image types the logo renderer understands.
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());
    if logo_mime(&extension).is_none() {
        return Err(format!("Unsupported logo format: .{}", extension));
    }

    let business_dir = get_business_dir();
    fs::create_dir_all(&business_dir)
        .map_err(|e| format!("Failed to create business directory: {}", e))?;

    // Generate unique filename to avoid conflicts
    let filename = format!("logo_{}.{}", uuid::Uuid::new_v4(), extension);
    let dest_path = business_dir.join(&filename);

    fs::copy(source, &dest_path).map_err(|e| format!("Failed to copy logo file: {}", e))?;

    // Return relative path from data directory
    Ok(format!("business/{}", filename))
}

fn logo_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "ico" => Some("image/x-icon"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "bmp" => Some("image/bmp"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Get business logo as base64 data URL
///
/// `logo_path` must name a file inside `data/business/`.
#[tauri::command(async)]
#[specta::specta]
pub fn get_business_logo(logo_path: String) -> Result<Option<String>, String> {
    if logo_path.is_empty() {
        return Ok(None);
    }

    let file_name = logo_path
        .strip_prefix("business/")
        .or_else(|| logo_path.strip_prefix("business\\"))
        .unwrap_or(&logo_path);
    if file_name.is_empty() || file_name.contains(['/', '\\', ':']) || file_name.contains("..") {
        return Err("Invalid logo path".to_string());
    }

    let full_path = get_business_dir().join(file_name);
    if !full_path.is_file() {
        return Ok(None);
    }

    let extension = full_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = logo_mime(&extension).ok_or_else(|| "Unsupported logo format".to_string())?;

    let data = fs::read(&full_path).map_err(|e| format!("Failed to read logo: {}", e))?;

    use base64::Engine;
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&data);

    Ok(Some(format!("data:{};base64,{}", mime, base64_data)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_nested_value() {
        let s = AppSettings::default();
        let s = set_setting_path(&s, "appearance.theme", Value::from("dark")).unwrap();
        assert_eq!(s.appearance.theme, crate::types::ThemeMode::Dark);
        assert!(set_setting_path(&s, "appearance.theme", Value::from("purple")).is_err());
        let s = set_setting_path(&s, "agent.apiKeys.openai", Value::from("sk-test")).unwrap();
        assert_eq!(s.agent.api_keys.openai.as_deref(), Some("sk-test"));
        let s = set_setting_path(&s, "agent.baseUrl", Value::from("http://x")).unwrap();
        assert_eq!(s.agent.base_url.as_deref(), Some("http://x"));
        let s = set_setting_path(&s, "agent.baseUrl", Value::Null).unwrap();
        assert_eq!(s.agent.base_url, None);
    }

    #[test]
    fn rejects_unknown_keys_and_wrong_types() {
        let s = AppSettings::default();
        assert!(set_setting_path(&s, "appearance.thme", Value::from("dark")).is_err());
        assert!(set_setting_path(&s, "appearance.enableAnimations", Value::from("yes")).is_err());
        assert!(set_setting_path(&s, "appearance..theme", Value::from("dark")).is_err());
    }

    #[test]
    fn recovery_keeps_readable_sections() {
        let dir = std::env::temp_dir().join(format!("rs-settings-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let content = r#"{
            "version": "0.6.0",
            "business": { "name": "Shop", "enabled": true },
            "agent": { "apiKeys": { "openai": "sk-keep" } },
            "reports": { "reportRetentionDays": "not a number" }
        }"#;
        fs::write(&path, content).unwrap();
        let s = recover_settings(&path, content, "test");
        assert_eq!(s.business.name, "Shop");
        assert_eq!(s.agent.api_keys.openai.as_deref(), Some("sk-keep"));
        assert_eq!(s.reports.report_retention_days, 0);
        assert!(s.reports.auto_save_reports);
        let backups = fs::read_dir(&dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("settings.corrupt-")
            })
            .count();
        assert_eq!(backups, 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
