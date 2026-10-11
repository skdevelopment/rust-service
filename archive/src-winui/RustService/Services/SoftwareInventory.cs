using Microsoft.Win32;

namespace RustService.Services;

public sealed class InstalledProgramEntry
{
    public string Name { get; set; } = "";
    public string Version { get; set; } = "";
    public string Publisher { get; set; } = "";
    public string InstallDate { get; set; } = "";
    public ulong SizeKb { get; set; }
    public bool SystemComponent { get; set; }
    public bool IsUpdate { get; set; }
}

public static class SoftwareInventory
{
    public static IReadOnlyList<InstalledProgramEntry> List(bool includeUpdates)
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var list = new List<InstalledProgramEntry>();
        foreach (var entry in ReadUninstallKeys())
        {
            if (string.IsNullOrWhiteSpace(entry.Name)) continue;
            if (!includeUpdates && (entry.SystemComponent || entry.IsUpdate)) continue;
            if (!seen.Add(entry.Name)) continue;
            list.Add(entry);
        }

        return list.OrderByDescending(e => e.SizeKb).ToList();
    }

    public static IEnumerable<InstalledProgramEntry> ReadUninstallKeys()
    {
        foreach (var entry in ReadHive(RegistryHive.LocalMachine, RegistryView.Registry64)) yield return entry;
        foreach (var entry in ReadHive(RegistryHive.LocalMachine, RegistryView.Registry32)) yield return entry;
        foreach (var entry in ReadHive(RegistryHive.CurrentUser, RegistryView.Default)) yield return entry;
    }

    private static IEnumerable<InstalledProgramEntry> ReadHive(RegistryHive hive, RegistryView view)
    {
        RegistryKey? baseKey = null;
        try
        {
            baseKey = RegistryKey.OpenBaseKey(hive, view);
        }
        catch
        {
            yield break;
        }

        using (baseKey)
        {
            using var uninstall = baseKey.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall");
            if (uninstall is null) yield break;
            foreach (var subName in uninstall.GetSubKeyNames())
            {
                using var sub = uninstall.OpenSubKey(subName);
                if (sub is null) continue;
                var name = sub.GetValue("DisplayName")?.ToString();
                if (string.IsNullOrWhiteSpace(name)) continue;
                var releaseType = sub.GetValue("ReleaseType")?.ToString() ?? "";
                var parent = sub.GetValue("ParentKeyName")?.ToString();
                var system = Convert.ToInt32(sub.GetValue("SystemComponent") ?? 0) == 1;
                var isUpdate = releaseType is "Update" or "Security Update" or "Hotfix"
                    || name.StartsWith("KB", StringComparison.OrdinalIgnoreCase)
                    || !string.IsNullOrWhiteSpace(parent);
                ulong size = 0;
                try { size = Convert.ToUInt64(sub.GetValue("EstimatedSize") ?? 0); } catch { /* ignore */ }
                yield return new InstalledProgramEntry
                {
                    Name = name,
                    Version = sub.GetValue("DisplayVersion")?.ToString() ?? "",
                    Publisher = sub.GetValue("Publisher")?.ToString() ?? "",
                    InstallDate = sub.GetValue("InstallDate")?.ToString() ?? "",
                    SizeKb = size,
                    SystemComponent = system,
                    IsUpdate = isUpdate
                };
            }
        }
    }
}
