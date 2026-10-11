namespace RustService.Services;

/// <summary>
/// Process-wide service host. Must call <see cref="Initialize"/> before use so
/// the portable data directory exists before SQLite/settings open.
/// </summary>
public static class AppServices
{
    private static readonly object Sync = new();

    public static SettingsStore Settings { get; private set; } = null!;
    public static ProgramStore Programs { get; private set; } = null!;
    public static ScriptStore Scripts { get; private set; } = null!;
    public static RequiredProgramService RequiredPrograms { get; private set; } = null!;
    public static SystemInfoService SystemInfo { get; private set; } = null!;
    public static NetworkDiagnosticsService Network { get; private set; } = null!;
    public static StartupService Startup { get; private set; } = null!;
    public static EventLogViewer EventLogs { get; private set; } = null!;
    public static BluescreenService Bluescreen { get; private set; } = null!;
    public static ReportStore Reports { get; private set; } = null!;
    public static ServiceRunner Runner { get; private set; } = null!;
    public static AgentMemory Memory { get; private set; } = null!;
    public static AgentHost Agent { get; private set; } = null!;
    public static AgentFiles Files { get; private set; } = null!;
    public static McpServer Mcp { get; private set; } = null!;
    public static TimeTrackingService TimeTracking { get; private set; } = null!;

    public static bool IsInitialized { get; private set; }

    public static void Initialize(string? dataRoot = null, bool startMcp = false)
    {
        lock (Sync)
        {
            ShutdownUnlocked();
            Core.DataDirectory.Ensure(dataRoot);
            Settings = new SettingsStore();
            Settings.Load();
            Programs = new ProgramStore();
            Scripts = new ScriptStore();
            RequiredPrograms = new RequiredProgramService(Settings);
            SystemInfo = new SystemInfoService();
            Network = new NetworkDiagnosticsService();
            Startup = new StartupService();
            EventLogs = new EventLogViewer();
            Bluescreen = new BluescreenService();
            Reports = new ReportStore();
            Runner = new ServiceRunner();
            Memory = new AgentMemory();
            Agent = new AgentHost();
            Files = new AgentFiles();
            Mcp = new McpServer();
            TimeTracking = new TimeTrackingService();
            IsInitialized = true;
            if (startMcp && Settings.Current.Agent.McpServerEnabled)
            {
                Mcp.Start();
            }
        }
    }

    public static void StartMcpIfEnabled()
    {
        if (!IsInitialized) return;
        if (Settings.Current.Agent.McpServerEnabled && !Mcp.IsRunning)
        {
            Mcp.Start();
        }
    }

    public static void Shutdown()
    {
        lock (Sync) ShutdownUnlocked();
    }

    private static void ShutdownUnlocked()
    {
        try { Mcp?.Dispose(); } catch { /* ignore */ }
        try { Memory?.Dispose(); } catch { /* ignore */ }
        Mcp = null!;
        Memory = null!;
        IsInitialized = false;
    }
}
