//! Application logging
//!
//! Logs go to stderr and to `data/logs/rustservice.log`. The verbosity follows
//! the `data.logLevel` setting and is updated live when that setting changes.

use log::LevelFilter;
use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use crate::commands::data_dir::get_data_dir_path;
use crate::types::{DataSettings, LogLevel};

/// Map the `data.logLevel` setting to a level filter.
pub fn level_from_setting(level: LogLevel) -> LevelFilter {
    match level {
        LogLevel::Error => LevelFilter::Error,
        LogLevel::Warn => LevelFilter::Warn,
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Debug => LevelFilter::Debug,
    }
}

/// Build the log plugin. The plugin itself admits everything up to `Trace`;
/// the effective verbosity is the global max level set by [`apply_settings`],
/// which must run after plugin setup (the plugin resets the max level).
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stderr),
            Target::new(TargetKind::Folder {
                path: get_data_dir_path().join("logs"),
                file_name: Some("rustservice".into()),
            }),
        ])
        .rotation_strategy(RotationStrategy::KeepOne)
        .max_file_size(5 * 1024 * 1024)
        .level(LevelFilter::Trace)
        // Dependencies are chatty at debug level; keep them at warn.
        .level_for("hyper", LevelFilter::Warn)
        .level_for("reqwest", LevelFilter::Warn)
        .level_for("tao", LevelFilter::Warn)
        .level_for("wry", LevelFilter::Warn)
        .build()
}

/// Apply the configured verbosity to the running logger.
pub fn apply_settings(data: &DataSettings) {
    log::set_max_level(level_from_setting(data.log_level));
}
