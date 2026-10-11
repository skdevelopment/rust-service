using System.Text.Json;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

/// <summary>
/// C# counterparts of every Tauri invoke_handler command. WinUI pages and MCP call these.
/// </summary>
public static class Commands
{
    public static readonly string[] TauriCommandNames =
    [
        "get_data_dir", "ensure_data_dir", "get_settings", "save_settings", "update_setting",
        "open_folder", "open_shortcut", "get_system_info", "test_network_latency",
        "get_programs", "add_program", "update_program", "delete_program", "launch_program",
        "extract_program_icon", "reveal_program", "get_program_icon", "import_legacy_programs",
        "get_scripts", "add_script", "update_script", "delete_script", "run_script",
        "list_usb_drives", "get_service_definitions", "get_service_presets", "validate_service_requirements",
        "get_service_run_state", "run_services", "cancel_service_run", "pause_service_run", "resume_service_run",
        "get_service_report", "list_service_reports", "delete_report", "clear_all_reports",
        "edit_report_finding", "add_report_finding", "remove_report_finding", "set_report_summary",
        "set_service_analysis", "set_report_health_score", "get_report_statistics", "generate_report_pdf",
        "save_service_preset", "delete_service_preset", "get_builtin_preset_ids",
        "get_required_programs", "get_required_programs_by_ids", "get_required_programs_status",
        "set_program_path_override", "get_program_exe_path", "validate_required_programs",
        "save_business_logo", "get_business_logo",
        "get_service_time_metrics", "record_service_time", "get_pc_fingerprint", "get_estimated_time",
        "get_service_averages", "get_preset_averages", "clear_service_metrics", "retrain_time_models", "flush_service_metrics",
        "get_detailed_network_info", "ping_host", "trace_route", "dns_lookup", "get_wifi_info",
        "get_startup_items", "toggle_startup_item", "delete_startup_item", "open_startup_item_location",
        "get_event_log_sources", "get_event_logs", "get_event_log_stats", "search_event_logs",
        "get_bsod_history", "get_bsod_details", "get_bsod_stats", "delete_crash_dump",
        "queue_agent_command", "execute_agent_command", "get_pending_commands", "clear_pending_commands",
        "approve_command", "reject_command", "search_tavily", "search_searxng", "get_agent_settings",
        "get_command_history", "agent_read_file", "agent_write_file", "agent_list_dir", "agent_move_file",
        "agent_copy_file", "agent_edit_file", "agent_grep", "agent_glob", "list_instruments",
        "list_agent_programs", "agent_find_exe",
        "save_uploaded_file", "generate_agent_file", "read_file_content", "read_file_binary",
        "get_file_info", "list_agent_files", "delete_agent_file", "validate_filesystem_path", "read_filesystem_file",
        "create_conversation", "list_conversations", "get_conversation", "save_conversation_messages",
        "update_conversation_title", "delete_conversation",
        "get_disk_health", "get_restore_points", "create_restore_point"
    ];

    public static string GetDataDir() => DataDirectory.Root;

    public static string EnsureDataDir()
    {
        DataDirectory.Ensure(DataDirectory.Root);
        return DataDirectory.Root;
    }

    public static AppSettings GetSettings() => AppServices.Settings.Current;

    public static void SaveSettings() => AppServices.Settings.Save();

    public static void UpdateSetting(string key, JsonElement value) => AppServices.Settings.UpdateSetting(key, value);

    public static void OpenFolder(string path) => ProcessRunner.OpenFolder(path);

    public static void OpenShortcut(string command, IEnumerable<string>? args = null, bool runAsAdmin = false) =>
        ProcessRunner.StartDetached(command, args, runAsAdmin);

    public static MachineSystemInfo GetSystemInfo() => AppServices.SystemInfo.Get();

    public static Task<PingResult> TestNetworkLatency(string host = "8.8.8.8") => AppServices.Network.PingAsync(host, 4);

    public static IReadOnlyList<ProgramItem> GetPrograms() => AppServices.Programs.GetAll();

    public static ProgramItem AddProgram(string name, string description, string version, string exePath, bool isCli) =>
        AppServices.Programs.Add(name, description, version, exePath, isCli);

    public static ProgramItem? UpdateProgram(ProgramItem item) => AppServices.Programs.Update(item);

    public static bool DeleteProgram(string id) => AppServices.Programs.Delete(id);

    public static ProgramItem? LaunchProgram(string id) => AppServices.Programs.Launch(id);

    public static string? ExtractProgramIcon(string exePath, string id) => AppServices.Programs.ExtractIcon(exePath, id);

    public static void RevealProgram(string id) => AppServices.Programs.Reveal(id);

    public static string? GetProgramIcon(string? relative) => AppServices.Programs.GetIconAbsolute(relative);

    public static int ImportLegacyPrograms() => AppServices.Programs.ImportLegacy();

    public static IReadOnlyList<ScriptItem> GetScripts() => AppServices.Scripts.GetAll();

    public static ScriptItem AddScript(ScriptItem item) => AppServices.Scripts.Add(item);

    public static ScriptItem? UpdateScript(ScriptItem item) => AppServices.Scripts.Update(item);

    public static bool DeleteScript(string id) => AppServices.Scripts.Delete(id);

    public static Task<ProcessResult> RunScript(string id) => AppServices.Scripts.RunAsync(id);

    public static IReadOnlyList<UsbDriveInfo> ListUsbDrives() =>
        DriveInfo.GetDrives().Where(d => d.IsReady && d.DriveType == DriveType.Removable)
            .Select(d => new UsbDriveInfo { Name = d.VolumeLabel, Path = d.Name, TotalBytes = d.TotalSize, FreeBytes = d.AvailableFreeSpace })
            .ToList();

    public static IReadOnlyList<ServiceDefinition> GetServiceDefinitions() => ServiceCatalog.Definitions;

    public static IReadOnlyList<ServicePreset> GetServicePresets() => ServiceCatalog.AllPresets();

    public static IReadOnlyList<string> ValidateServiceRequirements(IEnumerable<string> serviceIds)
    {
        var needed = serviceIds
            .Select(id => ServiceCatalog.Get(id))
            .Where(d => d is not null)
            .SelectMany(d => d!.RequiredPrograms)
            .Distinct();
        return AppServices.RequiredPrograms.Validate(needed);
    }

    public static ServiceRunState GetServiceRunState() => AppServices.Runner.State;

    public static Task<ServiceReport> RunServices(IReadOnlyList<ServiceQueueItem> queue, bool parallel, string? technician = null, string? customer = null) =>
        AppServices.Runner.RunAsync(queue, parallel, technician, customer);

    public static void CancelServiceRun() => AppServices.Runner.Cancel();

    public static void PauseServiceRun() => AppServices.Runner.Pause();

    public static void ResumeServiceRun() => AppServices.Runner.Resume();

    public static ServiceReport? GetServiceReport(string id) => AppServices.Reports.Get(id);

    public static IReadOnlyList<ServiceReport> ListServiceReports() => AppServices.Reports.List();

    public static bool DeleteReport(string id) => AppServices.Reports.Delete(id);

    public static void ClearAllReports() => AppServices.Reports.Clear();

    public static void EditReportFinding(string reportId, string serviceId, int findingIndex, string? severity, string? title, string? description, string? recommendation) =>
        AppServices.Reports.EditFinding(reportId, serviceId, findingIndex, severity, title, description, recommendation);

    public static void AddReportFinding(string reportId, string serviceId, string severity, string title, string description, string? recommendation) =>
        AppServices.Reports.AddFinding(reportId, serviceId, severity, title, description, recommendation);

    public static void RemoveReportFinding(string reportId, string serviceId, int findingIndex) =>
        AppServices.Reports.RemoveFinding(reportId, serviceId, findingIndex);

    public static void SetReportSummary(string reportId, string summary) => AppServices.Reports.SetSummary(reportId, summary);

    public static void SetServiceAnalysis(string reportId, string serviceId, string analysis) =>
        AppServices.Reports.SetAnalysis(reportId, serviceId, analysis);

    public static void SetReportHealthScore(string reportId, int score) => AppServices.Reports.SetHealthScore(reportId, score);

    public static ReportStatistics GetReportStatistics(string reportId)
    {
        var report = AppServices.Reports.Get(reportId) ?? throw new InvalidOperationException("Report not found");
        return AppServices.Reports.Statistics(report);
    }

    public static string GenerateReportPdf(string reportId, string? outputPath = null) =>
        AppServices.Reports.GeneratePdf(reportId, outputPath);

    public static void SaveServicePreset(ServicePreset preset)
    {
        var list = AppServices.Settings.Current.Presets.CustomPresets;
        var i = list.FindIndex(p => p.Id == preset.Id);
        if (i >= 0) list[i] = preset;
        else list.Add(preset);
        AppServices.Settings.Save();
    }

    public static void DeleteServicePreset(string id)
    {
        AppServices.Settings.Current.Presets.CustomPresets.RemoveAll(p => p.Id == id);
        AppServices.Settings.Save();
    }

    public static IReadOnlyList<string> GetBuiltinPresetIds() => ServiceCatalog.BuiltinPresetIds;

    public static IReadOnlyList<RequiredProgramDef> GetRequiredPrograms() => AppServices.RequiredPrograms.GetAll();

    public static IReadOnlyList<RequiredProgramStatus> GetRequiredProgramsByIds(IEnumerable<string> ids) =>
        AppServices.RequiredPrograms.GetStatus(ids);

    public static IReadOnlyList<RequiredProgramStatus> GetRequiredProgramsStatus() => AppServices.RequiredPrograms.GetStatus();

    public static void SetProgramPathOverride(string programId, string? path) =>
        AppServices.RequiredPrograms.SetOverride(programId, path);

    public static string? GetProgramExePath(string id) => AppServices.RequiredPrograms.GetExePath(id);

    public static IReadOnlyList<string> ValidateRequiredPrograms(IEnumerable<string> ids) =>
        AppServices.RequiredPrograms.Validate(ids);

    public static string? SaveBusinessLogo(byte[] bytes, string fileName) =>
        AppServices.Settings.SaveBusinessLogo(bytes, fileName);

    public static string? GetBusinessLogo()
    {
        var relative = AppServices.Settings.Current.Business.LogoPath;
        if (string.IsNullOrWhiteSpace(relative)) return null;
        var path = Path.IsPathRooted(relative) ? relative : DataDirectory.PathTo(relative.Replace('/', Path.DirectorySeparatorChar));
        return File.Exists(path) ? path : null;
    }

    public static ServiceTimeMetrics GetServiceTimeMetrics() => AppServices.TimeTracking.Get();

    public static void RecordServiceTime(string serviceId, long durationMs, string? presetId = null) =>
        AppServices.TimeTracking.Record(serviceId, durationMs, presetId);

    public static PcFingerprint GetPcFingerprint() => AppServices.TimeTracking.GetFingerprint();

    public static long GetEstimatedTime(string serviceId) => AppServices.TimeTracking.GetEstimatedTime(serviceId);

    public static IReadOnlyList<ServiceTimeStats> GetServiceAverages() => AppServices.TimeTracking.GetServiceAverages();

    public static IReadOnlyList<PresetTimeStats> GetPresetAverages() => AppServices.TimeTracking.GetPresetAverages();

    public static void ClearServiceMetrics() => AppServices.TimeTracking.Clear();

    public static void RetrainTimeModels() => AppServices.TimeTracking.Retrain();

    public static void FlushServiceMetrics() => AppServices.TimeTracking.Flush();

    public static IReadOnlyList<NetworkInterfaceDetails> GetDetailedNetworkInfo() => AppServices.Network.GetInterfaces();

    public static Task<PingResult> PingHost(string host, int count = 4) => AppServices.Network.PingAsync(host, count);

    public static Task<TracerouteResult> TraceRoute(string host) => AppServices.Network.TraceAsync(host);

    public static Task<DnsLookupResult> DnsLookup(string query, string type = "A") => AppServices.Network.LookupAsync(query, type);

    public static Task<WifiInfo> GetWifiInfo() => AppServices.Network.GetWifiAsync();

    public static IReadOnlyList<StartupItem> GetStartupItems() => AppServices.Startup.GetAll();

    public static void ToggleStartupItem(string id, bool enabled) => AppServices.Startup.Toggle(id, enabled);

    public static void DeleteStartupItem(string id) => AppServices.Startup.Delete(id);

    public static void OpenStartupItemLocation(string id) => AppServices.Startup.OpenLocation(id);

    public static IReadOnlyList<EventLogSource> GetEventLogSources() => AppServices.EventLogs.GetSources();

    public static IReadOnlyList<Models.EventLogEntry> GetEventLogs(string logName, string? level = null, string? source = null, int limit = 200, string? keyword = null) =>
        AppServices.EventLogs.Query(logName, level, source, keyword, limit);

    public static EventLogStats GetEventLogStats(string logName) => AppServices.EventLogs.Stats(logName);

    public static IReadOnlyList<Models.EventLogEntry> SearchEventLogs(string logName, string keyword) =>
        AppServices.EventLogs.Query(logName, keyword: keyword);

    public static IReadOnlyList<BsodEntry> GetBsodHistory() => AppServices.Bluescreen.History();

    public static BsodDetails GetBsodDetails(string dumpPath) => AppServices.Bluescreen.Details(dumpPath);

    public static BsodStats GetBsodStats() => AppServices.Bluescreen.Stats();

    public static void DeleteCrashDump(string path) => AppServices.Bluescreen.DeleteDump(path);

    public static PendingCommand QueueAgentCommand(string command, string reason) => AppServices.Agent.Queue(command, reason);

    public static Task<PendingCommand> ExecuteAgentCommand(string command, string reason) =>
        AppServices.Agent.ExecuteDirectAsync(command, reason);

    public static IReadOnlyList<PendingCommand> GetPendingCommands() => AppServices.Agent.Pending.ToList();

    public static void ClearPendingCommands() => AppServices.Agent.ClearPending();

    public static Task<PendingCommand> ApproveCommand(string commandId) => AppServices.Agent.ApproveAsync(commandId);

    public static PendingCommand RejectCommand(string commandId) => AppServices.Agent.Reject(commandId);

    public static Task<IReadOnlyList<SearchResult>> SearchTavily(string query, string apiKey) =>
        AppServices.Agent.SearchTavilyAsync(query, apiKey);

    public static Task<IReadOnlyList<SearchResult>> SearchSearxng(string query, string instanceUrl) =>
        AppServices.Agent.SearchSearxngAsync(query, instanceUrl);

    public static AgentSettings GetAgentSettings() => AppServices.Settings.Current.Agent;

    public static IReadOnlyList<PendingCommand> GetCommandHistory(int limit = 100) => AppServices.Memory.History(limit);

    public static string AgentReadFile(string path) => AppServices.Files.Read(path);

    public static string AgentWriteFile(string path, string content)
    {
        AppServices.Files.Write(path, content);
        return path;
    }

    public static IReadOnlyList<string> AgentListDir(string path) => AppServices.Files.ListDir(path);

    public static void AgentMoveFile(string src, string dest) => AppServices.Files.Move(src, dest);

    public static void AgentCopyFile(string src, string dest) => AppServices.Files.Copy(src, dest);

    public static void AgentEditFile(string path, string oldText, string newText) => AppServices.Files.Edit(path, oldText, newText);

    public static IReadOnlyList<string> AgentGrep(string root, string pattern) => AppServices.Files.Grep(root, pattern);

    public static IReadOnlyList<string> AgentGlob(string root, string pattern) => AppServices.Files.Glob(root, pattern);

    public static IReadOnlyList<string> ListInstruments() => AppServices.Files.ListInstruments();

    public static IReadOnlyList<string> ListAgentPrograms() => AppServices.Files.ListAgentPrograms();

    public static string? AgentFindExe(string name) => AppServices.Files.FindExe(name);

    public static FileAttachment SaveUploadedFile(string fileName, string mime, string contentBase64) =>
        AppServices.Files.SaveUpload(fileName, mime, contentBase64);

    public static FileAttachment GenerateAgentFile(string filename, string content, string description) =>
        AppServices.Files.Generate(filename, content, description);

    public static string ReadFileContent(string path) => AppServices.Files.Read(path);

    public static string ReadFileBinary(string path) => AppServices.Files.ReadBinaryBase64(path);

    public static object GetFileInfo(string path) => AppServices.Files.GetFileInfo(path);

    public static IReadOnlyList<FileAttachment> ListAgentFiles() => AppServices.Files.List();

    public static bool DeleteAgentFile(string id) => AppServices.Files.Delete(id);

    public static PathValidationResult ValidateFilesystemPath(string path) => AppServices.Files.ValidatePath(path);

    public static string ReadFilesystemFile(string path) => AppServices.Files.Read(path);

    public static Conversation CreateConversation(string title = "New conversation") => AppServices.Memory.CreateConversation(title);

    public static IReadOnlyList<Conversation> ListConversations() => AppServices.Memory.ListConversations();

    public static ConversationWithMessages? GetConversation(string id) => AppServices.Memory.GetConversation(id);

    public static void SaveConversationMessages(string conversationId, IEnumerable<ConversationMessage> messages) =>
        AppServices.Memory.SaveMessages(conversationId, messages);

    public static void UpdateConversationTitle(string id, string title) => AppServices.Memory.UpdateTitle(id, title);

    public static void DeleteConversation(string id) => AppServices.Memory.DeleteConversation(id);

    public static IReadOnlyList<DiskHealthInfo> GetDiskHealth() => AppServices.SystemInfo.GetDiskHealth();

    public static IReadOnlyList<RestorePointInfo> GetRestorePoints() => AppServices.SystemInfo.GetRestorePoints();

    public static Task<string> CreateRestorePoint(string description) => AppServices.SystemInfo.CreateRestorePointAsync(description);

    public static Task<ToolExecutionResponse> ExecuteCommand(string command, string reason) =>
        AppServices.Agent.ExecuteOrQueueAsync(command, reason);

    public static Task<IReadOnlyList<SearchResult>> SearchWeb(string query) => AppServices.Agent.SearchAsync(query);
}
