namespace RustService.Services;

public static class TauriCommandNames
{
    public static readonly string[] All =
    [
        "get_data_dir","ensure_data_dir","get_settings","save_settings","update_setting","open_folder","open_shortcut",
        "get_system_info","test_network_latency","get_programs","add_program","update_program","delete_program","launch_program",
        "extract_program_icon","reveal_program","get_program_icon","import_legacy_programs","get_scripts","add_script","update_script",
        "delete_script","run_script","list_usb_drives","get_service_definitions","get_service_presets","validate_service_requirements",
        "get_service_run_state","run_services","cancel_service_run","pause_service_run","resume_service_run","get_service_report",
        "list_service_reports","delete_report","clear_all_reports","edit_report_finding","add_report_finding","remove_report_finding",
        "set_report_summary","set_service_analysis","set_report_health_score","get_report_statistics","generate_report_pdf",
        "save_service_preset","delete_service_preset","get_builtin_preset_ids","get_required_programs","get_required_programs_by_ids",
        "get_required_programs_status","set_program_path_override","get_program_exe_path","validate_required_programs",
        "save_business_logo","get_business_logo","get_service_time_metrics","record_service_time","get_pc_fingerprint",
        "get_estimated_time","get_service_averages","get_preset_averages","clear_service_metrics","retrain_time_models",
        "flush_service_metrics","get_detailed_network_info","ping_host","trace_route","dns_lookup","get_wifi_info",
        "get_startup_items","toggle_startup_item","delete_startup_item","open_startup_item_location","get_event_log_sources",
        "get_event_logs","get_event_log_stats","search_event_logs","get_bsod_history","get_bsod_details","get_bsod_stats",
        "delete_crash_dump","queue_agent_command","execute_agent_command","get_pending_commands","clear_pending_commands",
        "approve_command","reject_command","search_tavily","search_searxng","get_agent_settings","get_command_history",
        "agent_read_file","agent_write_file","agent_list_dir","agent_move_file","agent_copy_file","agent_edit_file","agent_grep",
        "agent_glob","list_instruments","list_agent_programs","agent_find_exe","save_uploaded_file","generate_agent_file",
        "read_file_content","read_file_binary","get_file_info","list_agent_files","delete_agent_file","validate_filesystem_path",
        "read_filesystem_file","create_conversation","list_conversations","get_conversation","save_conversation_messages",
        "update_conversation_title","delete_conversation","get_disk_health","get_restore_points","create_restore_point"
    ];
}
