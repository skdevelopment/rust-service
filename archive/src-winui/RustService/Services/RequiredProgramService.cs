using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class RequiredProgramService
{
    private readonly SettingsStore settings;

    public RequiredProgramService(SettingsStore settings) => this.settings = settings;

    public static readonly IReadOnlyList<RequiredProgramDef> Registry =
    [
        new() { Id = "bleachbit", Name = "BleachBit", Description = "System cleaner to free disk space and maintain privacy", ExeNames = ["bleachbit.exe", "bleachbit_console.exe"], Url = "https://www.bleachbit.org/" },
        new() { Id = "adwcleaner", Name = "AdwCleaner", Description = "Adware and malware removal tool", ExeNames = ["adwcleaner.exe"], Url = "https://www.malwarebytes.com/adwcleaner" },
        new() { Id = "crystaldiskinfo", Name = "CrystalDiskInfo", Description = "Disk health monitoring utility", ExeNames = ["DiskInfo64.exe", "DiskInfo32.exe", "CrystalDiskInfo.exe"], Url = "https://crystalmark.info/en/software/crystaldiskinfo/" },
        new() { Id = "kvrt", Name = "Kaspersky Virus Removal Tool", Description = "Antivirus scanning and malware removal tool", ExeNames = ["KVRT.exe"], Url = "https://www.kaspersky.com/downloads/free-virus-removal-tool" },
        new() { Id = "whynotwin11", Name = "WhyNotWin11", Description = "Windows 11 compatibility checker", ExeNames = ["WhyNotWin11.exe", "WhyNotWin11Portable.exe"], Url = "https://github.com/rcmaehl/WhyNotWin11/releases" },
        new() { Id = "smartctl", Name = "Smartctl (smartmontools)", Description = "SMART disk health monitoring and reporting tool", ExeNames = ["smartctl.exe"], Url = "https://www.smartmontools.org/wiki/Download" },
        new() { Id = "speedtest", Name = "Speedtest CLI", Description = "Ookla Speedtest command-line interface for bandwidth testing", ExeNames = ["speedtest.exe"], Url = "https://www.speedtest.net/apps/cli" },
        new() { Id = "iperf3", Name = "iPerf3", Description = "Network performance and stability testing tool", ExeNames = ["iperf3.exe"], Url = "https://iperf.fr/iperf-download.php" },
        new() { Id = "drivecleanup", Name = "DriveCleanup", Description = "Remove stale USB devices and registry entries", ExeNames = ["DriveCleanup.exe"], Url = "https://www.uwe-sieber.de/drivetools_e.html" },
        new() { Id = "heavyload", Name = "HeavyLoad", Description = "CPU, memory, and disk stress testing tool", ExeNames = ["HeavyLoad.exe"], Url = "https://www.jam-software.com/heavyload" },
        new() { Id = "furmark", Name = "FurMark", Description = "GPU stress testing and stability tool", ExeNames = ["FurMark.exe", "FurMark_GUI.exe"], Url = "https://geeks3d.com/furmark/" },
        new() { Id = "stinger", Name = "Trellix Stinger", Description = "Standalone antivirus scanner for malware detection", ExeNames = ["stinger64.exe", "stinger32.exe", "stinger.exe"], Url = "https://www.trellix.com/products/stinger/" },
    ];

    public IReadOnlyList<RequiredProgramDef> GetAll() => Registry;

    public IReadOnlyList<RequiredProgramStatus> GetStatus(IEnumerable<string>? ids = null)
    {
        var wanted = ids?.ToHashSet(StringComparer.OrdinalIgnoreCase);
        return Registry
            .Where(d => wanted is null || wanted.Contains(d.Id))
            .Select(Resolve)
            .ToList();
    }

    public string? GetExePath(string id) => Resolve(Registry.FirstOrDefault(d => d.Id == id) ?? new RequiredProgramDef { Id = id }).Path;

    public void SetOverride(string programId, string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            settings.Current.Programs.Overrides.Remove(programId);
        }
        else
        {
            settings.Current.Programs.Overrides[programId] = path;
        }

        settings.Save();
    }

    public IReadOnlyList<string> Validate(IEnumerable<string> ids) =>
        GetStatus(ids).Where(s => !s.Found).Select(s => s.Definition.Id).ToList();

    public RequiredProgramStatus Resolve(RequiredProgramDef def)
    {
        if (settings.Current.Programs.Overrides.TryGetValue(def.Id, out var custom) && File.Exists(custom))
        {
            return new RequiredProgramStatus { Definition = def, Found = true, Path = custom, IsCustom = true };
        }

        var found = FindInPrograms(def.ExeNames);
        return new RequiredProgramStatus { Definition = def, Found = found is not null, Path = found, IsCustom = false };
    }

    private static string? FindInPrograms(IEnumerable<string> names)
    {
        var root = DataDirectory.PathTo("programs");
        if (!Directory.Exists(root)) return null;
        var set = names.ToHashSet(StringComparer.OrdinalIgnoreCase);
        return Directory.EnumerateFiles(root, "*.*", SearchOption.AllDirectories)
            .FirstOrDefault(path => set.Contains(Path.GetFileName(path)));
    }
}
