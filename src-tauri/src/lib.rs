//! RustService Backend Library
//!
//! Tauri backend for the RustService Windows desktop toolkit.
//! Handles system operations, data folder management, settings persistence,
//! and system information collection.
//!
//! # Module Structure
//!
//! - `types` - Data structures for settings and system information
//! - `commands` - Tauri command handlers exposed to the frontend
//! - `mcp` - Model Context Protocol server for remote LLM access

mod commands;
mod llm;
mod logging;
mod mcp;
mod proc;
mod services;
mod types;

// ============================================================================
// Application Entry Point
// ============================================================================

/// Path of the generated TypeScript bindings, relative to `src-tauri/`.
const BINDINGS_PATH: &str = "../src/bindings.ts";

/// All IPC commands, with their TypeScript bindings.
///
/// The frontend calls these only through the generated `src/bindings.ts`, so
/// argument names and types are checked by `tsc` instead of failing at runtime.
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        // Sizes and durations are u64 on the Rust side; they stay well below 2^53.
        .dangerously_cast_bigints_to_number()
        .commands(tauri_specta::collect_commands![
            commands::data_dir::get_data_dir,
            commands::data_dir::ensure_data_dir,
            commands::settings::get_settings,
            commands::settings::update_setting,
            commands::utils::open_folder,
            commands::shortcuts::open_shortcut,
            commands::system_info::get_system_info,
            commands::network::test_network_latency,
            commands::programs::get_programs,
            commands::programs::add_program,
            commands::programs::update_program,
            commands::programs::delete_program,
            commands::programs::launch_program,
            commands::programs::extract_program_icon,
            commands::programs::reveal_program,
            commands::programs::get_program_icon,
            commands::programs::import_legacy_programs,
            commands::scripts::get_scripts,
            commands::scripts::add_script,
            commands::scripts::update_script,
            commands::scripts::delete_script,
            commands::scripts::run_script,
            // Service commands
            commands::services::list_usb_drives,
            commands::services::get_service_definitions,
            commands::services::get_service_presets,
            commands::services::validate_service_requirements,
            commands::services::runner::get_service_run_state,
            commands::services::runner::run_services,
            commands::services::runner::cancel_service_run,
            commands::services::runner::pause_service_run,
            commands::services::runner::resume_service_run,
            commands::services::reports::get_service_report,
            commands::services::reports::list_service_reports,
            commands::services::reports::delete_report,
            commands::services::reports::clear_all_reports,
            // Report editing commands (agent)
            commands::services::reports::edit_report_finding,
            commands::services::reports::add_report_finding,
            commands::services::reports::remove_report_finding,
            commands::services::reports::set_report_summary,
            commands::services::reports::set_service_analysis,
            commands::services::reports::set_report_health_score,
            commands::services::reports::get_report_statistics,
            commands::services::reports::export_report_text,
            // Service presets management
            commands::services::save_service_preset,
            commands::services::delete_service_preset,
            commands::services::get_builtin_preset_ids,
            // Required programs commands
            commands::required_programs::get_required_programs,
            commands::required_programs::get_required_programs_by_ids,
            commands::required_programs::get_required_programs_status,
            commands::required_programs::set_program_path_override,
            commands::required_programs::get_program_exe_path,
            commands::required_programs::validate_required_programs,
            // Business logo commands
            commands::settings::save_business_logo,
            commands::settings::get_business_logo,
            // Service time tracking commands
            commands::time_tracking::get_service_time_metrics,
            commands::time_tracking::record_service_time,
            commands::time_tracking::get_pc_fingerprint,
            commands::time_tracking::get_estimated_time,
            commands::time_tracking::get_service_averages,
            commands::time_tracking::get_preset_averages,
            commands::time_tracking::clear_service_metrics,
            commands::time_tracking::retrain_time_models,
            commands::time_tracking::flush_service_metrics,
            // Network diagnostics commands
            commands::network_diagnostics::get_detailed_network_info,
            commands::network_diagnostics::ping_host,
            commands::network_diagnostics::trace_route,
            commands::network_diagnostics::dns_lookup,
            commands::network_diagnostics::get_wifi_info,
            // Startup manager commands
            commands::startup::get_startup_items,
            commands::startup::toggle_startup_item,
            commands::startup::delete_startup_item,
            commands::startup::open_startup_item_location,
            // Event log commands
            commands::event_log::get_event_log_sources,
            commands::event_log::get_event_logs,
            commands::event_log::get_event_log_stats,
            commands::event_log::search_event_logs,
            // Bluescreen analysis commands
            commands::bluescreen::get_bsod_history,
            commands::bluescreen::get_bsod_details,
            commands::bluescreen::get_bsod_stats,
            commands::bluescreen::delete_crash_dump,
            // Agent commands
            commands::agent::commands::execute_agent_command,
            commands::agent::commands::check_command_whitelisted,
            commands::agent::commands::run_agent_instrument,
            commands::agent::search::agent_search_web,
            commands::agent::files::agent_read_file,
            commands::agent::files::agent_write_file,
            commands::agent::files::agent_list_dir,
            commands::agent::files::agent_move_file,
            commands::agent::files::agent_copy_file,
            commands::agent::files::agent_edit_file,
            commands::agent::files::agent_grep,
            commands::agent::files::agent_glob,
            commands::agent::files::list_instruments,
            commands::agent::files::list_agent_programs,
            commands::agent::files::agent_find_exe,
            // File attachment commands
            commands::agent::attachments::save_uploaded_file,
            commands::agent::attachments::generate_agent_file,
            commands::agent::attachments::read_file_content,
            commands::agent::attachments::read_file_binary,
            commands::agent::attachments::get_file_info,
            commands::agent::attachments::open_agent_file,
            commands::agent::attachments::reveal_agent_file,
            commands::agent::attachments::list_agent_files,
            commands::agent::attachments::delete_agent_file,
            commands::agent::attachments::validate_filesystem_path,
            commands::agent::attachments::read_filesystem_file,
            // Conversation commands
            commands::agent::conversations::create_conversation,
            commands::agent::conversations::list_conversations,
            commands::agent::conversations::get_conversation,
            commands::agent::conversations::save_conversation_messages,
            commands::agent::conversations::update_conversation_title,
            commands::agent::conversations::delete_conversation,
            // Disk health commands
            commands::disk_health::get_disk_health,
            // Restore point commands
            commands::restore_points::get_restore_points,
            commands::restore_points::create_restore_point,
        ])
}

/// Write `src/bindings.ts` from the current command signatures.
///
/// Runs on a thread with a large stack: collecting ~130 command signatures
/// recurses deeply in debug builds and overflows Windows' 1 MB main stack.
pub fn export_bindings() {
    std::thread::Builder::new()
        .name("specta-export".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(export_bindings_now)
        .expect("failed to spawn bindings export thread")
        .join()
        .expect("bindings export panicked");
}

fn export_bindings_now() {
    specta_builder()
        .export(
            specta_typescript::Typescript::default()
                .header("// @ts-nocheck\n// This file is generated by tauri-specta from the Rust commands. Do not edit.\n"),
            BINDINGS_PATH,
        )
        .expect("failed to export TypeScript bindings");
}

/// Tauri application entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = commands::settings::get_settings().unwrap_or_default();
    let data_settings = settings.data.clone();

    let specta = specta_builder();
    #[cfg(debug_assertions)]
    export_bindings();

    tauri::Builder::default()
        .plugin(logging::plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |_app| {
            logging::apply_settings(&data_settings);
            mcp::apply_settings(&settings.agent);
            if let Err(e) =
                commands::services::reports::apply_retention(settings.reports.report_retention_days)
            {
                log::warn!("Report retention cleanup failed: {}", e);
            }
            Ok(())
        })
        .invoke_handler(specta.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
