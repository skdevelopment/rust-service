using System.Text.Json;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public static class ServiceCatalog
{
    public static IReadOnlyList<string> BuiltinPresetIds { get; } = ["diagnostics", "general", "complete", "custom"];

    public static IReadOnlyList<ServiceDefinition> Definitions { get; } = BuildDefinitions();

    public static ServiceDefinition? Get(string id) => Definitions.FirstOrDefault(d => d.Id == id);

    public static IReadOnlyList<ServicePreset> BuiltinPresets() =>
    [
        Preset("diagnostics", "Diagnostics", "Quick diagnostic tests to identify system issues", "stethoscope", "blue",
            C("ping-test", true, "{\"target\":\"8.8.8.8\",\"count\":4}"),
            C("speedtest", true),
            C("disk-space", true),
            C("smartctl", true),
            C("battery-report", true),
            C("whynotwin11", true),
            C("energy-report", true, "{\"duration\":10}"),
            C("network-config", true)),
        Preset("general", "General Service", "Standard maintenance tasks for regular checkups", "wrench", "green",
            C("restore-point", true),
            C("adwcleaner", true),
            C("kvrt-scan", true),
            C("ping-test", true, "{\"target\":\"8.8.8.8\",\"count\":4}"),
            C("speedtest", true),
            C("disk-space", true),
            C("smartctl", true),
            C("battery-report", true),
            C("network-config", true)),
        Preset("complete", "Complete Service", "Comprehensive scan and cleanup for thorough maintenance", "shield-check", "purple",
            C("restore-point", true),
            C("adwcleaner", true),
            C("kvrt-scan", true),
            C("bleachbit", true),
            C("drivecleanup", true, "{\"test_only\":false}"),
            C("ping-test", true, "{\"target\":\"8.8.8.8\",\"count\":10}"),
            C("speedtest", true),
            C("iperf", true, "{\"server\":\"iperf.he.net\",\"duration\":60,\"reverse\":true}"),
            C("disk-space", true),
            C("smartctl", true),
            C("battery-report", true),
            C("whynotwin11", true),
            C("winsat", true, "{\"drive\":\"C\"}"),
            C("dism", true, "{\"scan_only\":false}"),
            C("sfc", true),
            C("windows-update", true, "{\"install_updates\":true,\"include_drivers\":true}"),
            C("energy-report", true, "{\"duration\":10}"),
            C("driver-audit", true, "{\"show_all\":false}"),
            C("installed-software", true, "{\"include_updates\":false}"),
            C("network-config", true),
            C("usb-stability", false, "{\"test_intensity\":\"standard\",\"verify_integrity\":true}"),
            C("startup-optimize", true, "{\"disable_unnecessary\":false,\"include_disabled\":false}")),
        Preset("custom", "Custom Service", "Build your own service configuration", "sliders", "gray",
            C("restore-point", false),
            C("ping-test", false, "{\"target\":\"8.8.8.8\",\"count\":4}"),
            C("disk-space", false))
    ];

    public static IReadOnlyList<ServicePreset> AllPresets()
    {
        var custom = AppServices.Settings.Current.Presets.CustomPresets;
        var builtin = BuiltinPresets().ToList();
        foreach (var preset in custom)
        {
            var index = builtin.FindIndex(p => p.Id == preset.Id);
            if (index >= 0) builtin[index] = preset;
            else builtin.Add(preset);
        }

        return builtin;
    }

    private static ServicePreset Preset(string id, string name, string description, string icon, string color, params PresetServiceConfig[] services) =>
        new() { Id = id, Name = name, Description = description, Icon = icon, Color = color, Services = [.. services] };

    private static PresetServiceConfig C(string id, bool enabled, string json = "{}") =>
        new() { ServiceId = id, Enabled = enabled, Options = JsonDocument.Parse(json).RootElement.Clone() };

    private static List<ServiceDefinition> BuildDefinitions() =>
    [
        Def("restore-point", "System Restore Point", "Create a Windows restore point before changes", "maintenance", 20, "shield", [], []),
        Def("ping-test", "Ping Test", "Tests network connectivity by pinging specified hosts", "diagnostics", 10, "wifi", [], [],
            [Opt("target", "Target Host", "string", "\"8.8.8.8\""), Opt("count", "Ping Count", "number", "4", 1, 100)]),
        Def("disk-space", "Disk Space Analysis", "Analyze free space on all volumes", "diagnostics", 10, "hard-drive", [], []),
        Def("winsat", "Disk Performance Benchmark", "Run WinSAT disk assessment", "diagnostics", 90, "gauge", [], ["cpu-stress"],
            [Opt("drive", "Drive", "string", "\"C\"")]),
        Def("kvrt-scan", "Virus Scan (KVRT)", "Scan with Kaspersky Virus Removal Tool", "security", 600, "shield", ["kvrt"], ["filesystem-scan"]),
        Def("adwcleaner", "Adware Cleanup", "Remove adware with AdwCleaner", "security", 180, "bug", ["adwcleaner"], ["filesystem-scan"]),
        Def("stinger", "Antivirus Scan (Stinger)", "Scan with Trellix Stinger", "security", 300, "crosshair", ["stinger"], ["filesystem-scan"]),
        Def("whynotwin11", "Windows 11 Compatibility", "Check Windows 11 readiness", "diagnostics", 20, "monitor", ["whynotwin11"], []),
        Def("smartctl", "Drive Health Report", "Reports SMART health data for all drives", "diagnostics", 30, "activity", ["smartctl"], []),
        Def("speedtest", "Network Speed Test", "Measure internet bandwidth", "diagnostics", 40, "gauge", ["speedtest"], ["network-bandwidth"]),
        Def("iperf", "Network Stability Test", "iPerf3 throughput test", "diagnostics", 70, "network", ["iperf3"], ["network-bandwidth"],
            [Opt("server", "Server", "string", "\"iperf.he.net\""), Opt("duration", "Duration", "number", "60", 5, 300), Opt("reverse", "Reverse", "boolean", "true")]),
        Def("bleachbit", "System Cleanup", "Clean junk files with BleachBit", "cleanup", 120, "trash-2", ["bleachbit"], ["disk-heavy"]),
        Def("drivecleanup", "Device Cleanup", "Remove stale USB devices with DriveCleanup", "cleanup", 30, "usb", ["drivecleanup"], ["disk-heavy"],
            [Opt("test_only", "Test Only (don't remove)", "boolean", "false")]),
        Def("heavyload", "Stress Test", "CPU/RAM/disk stress with HeavyLoad", "diagnostics", 180, "cpu", ["heavyload"], ["cpu-stress"]),
        Def("dism", "Component Store Health", "Check and repair Windows component store using DISM", "maintenance", 300, "package-check", [], ["disk-heavy"],
            [Opt("scan_only", "Scan Only (no repair)", "boolean", "false")]),
        Def("sfc", "System File Check", "Scan and repair Windows system files using SFC", "maintenance", 600, "file-scan", [], ["disk-heavy"], deps: ["dism"]),
        Def("chkdsk", "Disk Check (CHKDSK)", "Check a volume for file system errors", "maintenance", 600, "disc", [], ["disk-exclusive"],
            [Opt("drive", "Drive", "string", "\"C\""), Opt("repair", "Repair", "boolean", "false")]),
        Def("furmark", "GPU Stress Test (FurMark)", "Stress test GPU with FurMark", "diagnostics", 120, "flame", ["furmark"], ["cpu-stress"],
            [Opt("duration_seconds", "Duration (seconds)", "number", "60", 10, 600)]),
        Def("windows-update", "Windows Update", "Scan and optionally install updates", "maintenance", 600, "download", [], [],
            [Opt("install_updates", "Install updates", "boolean", "true"), Opt("include_drivers", "Include drivers", "boolean", "true")]),
        Def("energy-report", "Energy Report", "Generate powercfg energy report", "diagnostics", 80, "battery", [], [],
            [Opt("duration", "Duration", "number", "10", 5, 120)]),
        Def("battery-report", "Battery Health Check", "Generate powercfg battery report", "diagnostics", 20, "battery-charging", [], []),
        Def("driver-audit", "Driver Audit", "List problem drivers", "diagnostics", 20, "cpu", [], [],
            [Opt("show_all", "Show all", "boolean", "false")]),
        Def("installed-software", "Installed Software Audit", "Inventory installed programs", "diagnostics", 30, "app-window", [], [],
            [Opt("include_updates", "Include updates", "boolean", "false")]),
        Def("network-config", "Network Configuration", "Capture adapter and IP configuration", "diagnostics", 15, "network", [], []),
        Def("usb-stability", "USB Stability Test", "Write/read test on a removable drive", "diagnostics", 120, "usb", [], ["disk-heavy"],
            [Opt("test_intensity", "Intensity", "string", "\"standard\""), Opt("verify_integrity", "Verify integrity", "boolean", "true")]),
        Def("startup-optimize", "Startup Optimizer", "Review and optionally disable heavy startup items", "maintenance", 20, "rocket", [], [],
            [Opt("disable_unnecessary", "Disable unnecessary", "boolean", "false"), Opt("include_disabled", "Include disabled", "boolean", "false")])
    ];

    private static ServiceDefinition Def(
        string id, string name, string description, string category, int secs, string icon,
        string[] required, string[] exclusive, ServiceOptionSchema[]? options = null, string[]? deps = null) =>
        new()
        {
            Id = id,
            Name = name,
            Description = description,
            Category = category,
            EstimatedDurationSecs = secs,
            Icon = icon,
            RequiredPrograms = [.. required],
            ExclusiveResources = [.. exclusive],
            Options = options?.ToList() ?? [],
            Dependencies = deps?.ToList() ?? []
        };

    private static ServiceOptionSchema Opt(string id, string label, string type, string json, double? min = null, double? max = null) =>
        new()
        {
            Id = id,
            Label = label,
            OptionType = type,
            DefaultValue = JsonDocument.Parse(json).RootElement.Clone(),
            Min = min,
            Max = max
        };
}
