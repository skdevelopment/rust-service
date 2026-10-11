//! Utility commands

/// Opens a folder in the system file explorer
///
/// Only existing directories are accepted: handing Explorer a file path would
/// open (or run) that file.
#[tauri::command(async)]
#[specta::specta]
pub fn open_folder(path: String) -> Result<(), String> {
    if !std::path::Path::new(&path).is_dir() {
        return Err(format!("Not a folder: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        crate::proc::command("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Fallback for other platforms (shouldn't be needed for Windows-only app)
        crate::proc::command("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}
