using RustService.Models;

namespace RustService.Services;

public static class ShortcutCatalog
{
    public static IReadOnlyList<ShortcutGroup> Groups { get; } =
    [
        new()
        {
            Id = "system",
            Name = "System",
            Items =
            [
                S("taskmgr", "Task Manager", "Monitor processes & performance", "taskmgr"),
                S("devmgmt", "Device Manager", "Hardware & drivers", "devmgmt.msc"),
                S("msconfig", "System Config", "Startup & boot options", "msconfig"),
                S("msinfo", "System Information", "Detailed system info", "msinfo32"),
                S("winver", "About Windows", "Windows version info", "winver"),
                S("resmon", "Resource Monitor", "Advanced resource usage", "resmon"),
                S("perfmon", "Performance Monitor", "Performance counters", "perfmon"),
                S("perfmon-report", "Performance Report", "60-second diagnostics report", "perfmon", [" /report"]),
                S("dxdiag", "DirectX Diagnostics", "Graphics & sound info", "dxdiag"),
                S("sys-adv", "Advanced System Settings", "Performance, profiles, startup", "SystemPropertiesAdvanced.exe"),
                S("sys-protect", "System Protection", "Restore points & protection", "SystemPropertiesProtection.exe"),
                S("admintools", "Windows Tools", "Administrative Tools folder", "control", ["admintools"]),
                S("verifier", "Driver Verifier", "Test driver stability", "verifier", admin: true)
            ]
        },
        new()
        {
            Id = "disk",
            Name = "Disk",
            Items =
            [
                S("diskmgmt", "Disk Management", "Partition & volume management", "diskmgmt.msc"),
                S("diskpart", "Diskpart", "Command-line disk utility", "cmd", ["/k", "diskpart"], true),
                S("defrag", "Defragment & Optimize", "Optimize drive performance", "dfrgui"),
                S("cleanmgr", "Disk Cleanup", "Free up disk space", "cleanmgr"),
                S("chkdsk", "Check Disk", "Scan for disk errors", "cmd", ["/k", "echo Run: chkdsk C: /f /r"], true),
                S("sfc", "System File Checker", "Repair system files", "cmd", ["/k", "sfc /scannow"], true),
                S("dism", "DISM Tool", "Repair Windows image", "cmd", ["/k", "DISM /Online /Cleanup-Image /RestoreHealth"], true)
            ]
        },
        new()
        {
            Id = "network",
            Name = "Network",
            Items =
            [
                S("ncpa", "Network Connections", "Manage network adapters", "ncpa.cpl"),
                S("firewall", "Windows Firewall", "Firewall with advanced security", "wf.msc"),
                S("rdp", "Remote Desktop", "Remote Desktop Connection", "mstsc"),
                S("ipconfig", "IP Config", "View network configuration", "cmd", ["/k", "ipconfig /all"]),
                S("ping", "Ping Test", "Ping 8.8.8.8", "cmd", ["/k", "ping 8.8.8.8"]),
                S("tracert", "Traceroute", "Trace route to 8.8.8.8", "cmd", ["/k", "tracert 8.8.8.8"]),
                S("flushdns", "Flush DNS", "Clear DNS cache", "cmd", ["/k", "ipconfig /flushdns"], true)
            ]
        },
        new()
        {
            Id = "controlpanel",
            Name = "Control Panel",
            Items =
            [
                S("control", "Control Panel", "Classic control panel", "control"),
                S("appwiz", "Programs & Features", "Installed programs", "appwiz.cpl"),
                S("powercfg", "Power Options", "Power plans & settings", "powercfg.cpl"),
                S("sysdm", "System Properties", "Computer name & advanced", "sysdm.cpl"),
                S("mmsys", "Sound Settings", "Audio devices", "mmsys.cpl"),
                S("timedate", "Date & Time", "Time zone & clock settings", "timedate.cpl")
            ]
        },
        new()
        {
            Id = "cmdline",
            Name = "Command line",
            Items =
            [
                S("cmd", "Command Prompt", "Windows command-line", "cmd"),
                S("cmd-admin", "CMD (Admin)", "Elevated command prompt", "cmd", admin: true),
                S("powershell", "PowerShell", "Windows PowerShell", "powershell"),
                S("powershell-admin", "PowerShell (Admin)", "Elevated PowerShell", "powershell", admin: true),
                S("wt", "Windows Terminal", "Modern terminal app", "wt"),
                S("netstat", "Netstat", "Active connections & ports", "cmd", ["/k", "netstat -ano"])
            ]
        },
        new()
        {
            Id = "advanced",
            Name = "Advanced",
            Items =
            [
                S("regedit", "Registry Editor", "Edit Windows registry", "regedit", admin: true),
                S("services", "Services", "Manage Windows services", "services.msc"),
                S("eventvwr", "Event Viewer", "System & app logs", "eventvwr.msc"),
                S("compmgmt", "Computer Management", "System tools hub", "compmgmt.msc"),
                S("gpedit", "Group Policy Editor", "Local policy settings", "gpedit.msc"),
                S("taskschd", "Task Scheduler", "Scheduled tasks", "taskschd.msc"),
                S("recovery", "Recovery Options", "Recovery environment", "rstrui")
            ]
        }
    ];

    private static ShortcutDef S(string id, string name, string description, string command, string[]? args = null, bool admin = false) =>
        new() { Id = id, Name = name, Description = description, Command = command, Args = args, AdminRequired = admin };
}
