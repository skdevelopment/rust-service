//! Data directory management commands

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::lock::lock_mutex;
use crate::types::AppSettings;

static DATA_DIR_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);
#[cfg(test)]
static DATA_DIR_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Override the data directory (tests). Pass `None` to restore the default.
#[cfg(test)]
pub fn set_data_dir_override(path: Option<PathBuf>) {
    *lock_mutex(&DATA_DIR_OVERRIDE) = path;
}

/// Run `f` with an exclusive data-dir override so parallel tests cannot clobber it.
#[cfg(test)]
pub fn with_exclusive_data_dir<R>(path: PathBuf, f: impl FnOnce() -> R) -> R {
    let _guard = lock_mutex(&DATA_DIR_TEST_LOCK);
    set_data_dir_override(Some(path));
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            set_data_dir_override(None);
        }
    }
    let _reset = Reset;
    f()
}

/// Gets the path to the data directory.
/// In production, this is adjacent to the executable.
/// In development, this is in the project root.
/// Tests may override this via [`set_data_dir_override`].
pub fn get_data_dir_path() -> PathBuf {
    if let Some(overridden) = lock_mutex(&DATA_DIR_OVERRIDE).clone() {
        return overridden;
    }

    // Try to get the executable's directory first (production)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let data_dir = exe_dir.join("data");
            // Check if we're in a development environment by looking for src-tauri
            // If we are, use the project root instead
            if !exe_dir.join("src-tauri").exists() && !exe_dir.to_string_lossy().contains("target")
            {
                return data_dir;
            }
        }
    }

    // Fallback to current directory (development)
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
}

/// Returns the path to the data directory
#[tauri::command(async)]
#[specta::specta]
pub fn get_data_dir() -> Result<String, String> {
    let data_dir = get_data_dir_path();
    Ok(data_dir.to_string_lossy().to_string())
}

/// Creates the data directory structure if it doesn't exist
#[tauri::command(async)]
#[specta::specta]
pub fn ensure_data_dir() -> Result<(), String> {
    let data_dir = get_data_dir_path();

    // Create main data directory and subdirectories
    let subdirs = ["programs", "logs", "reports", "scripts", "agent"];

    for subdir in subdirs {
        let path = data_dir.join(subdir);
        if !path.exists() {
            fs::create_dir_all(&path)
                .map_err(|e| format!("Failed to create directory {:?}: {}", path, e))?;
        }
    }

    // Create default settings file if it doesn't exist
    let settings_path = data_dir.join("settings.json");
    if !settings_path.exists() {
        let default_settings = AppSettings::default();
        let settings_json = serde_json::to_string_pretty(&default_settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        fs::write(&settings_path, settings_json)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;
    }

    Ok(())
}
