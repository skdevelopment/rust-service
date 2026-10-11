using Microsoft.Win32;
using Microsoft.Win32.TaskScheduler;
using RustService.Core;
using RustService.Models;
using Task = Microsoft.Win32.TaskScheduler.Task;

namespace RustService.Services;

public sealed class StartupService
{
    public const string ApprovedRunPath = @"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    public const string RunPath = @"Software\Microsoft\Windows\CurrentVersion\Run";

    public IReadOnlyList<StartupItem> GetAll()
    {
        var items = new List<StartupItem>();
        items.AddRange(FromRegistry(Registry.CurrentUser, RunPath, "registryCurrentUser"));
        items.AddRange(FromRegistry(Registry.LocalMachine, RunPath, "registryLocalMachine"));
        items.AddRange(FromFolder(Environment.GetFolderPath(Environment.SpecialFolder.Startup), "startupFolderUser"));
        items.AddRange(FromFolder(Environment.GetFolderPath(Environment.SpecialFolder.CommonStartup), "startupFolderAllUsers"));
        items.AddRange(FromTasks());
        return items;
    }

    public void Toggle(string id, bool enabled)
    {
        var item = GetAll().FirstOrDefault(i => i.Id == id) ?? throw new InvalidOperationException("Startup item not found");
        if (item.Source.StartsWith("registry", StringComparison.OrdinalIgnoreCase))
        {
            var hive = item.Source == "registryLocalMachine" ? Registry.LocalMachine : Registry.CurrentUser;
            using var approved = hive.CreateSubKey(ApprovedRunPath);
            approved?.SetValue(item.Name, enabled ? EnabledBytes() : DisabledBytes(), RegistryValueKind.Binary);
        }
        else if (item.Source.Contains("Folder", StringComparison.OrdinalIgnoreCase) && item.Path is not null)
        {
            if (!enabled && File.Exists(item.Path))
            {
                var disabledDir = Path.Combine(Path.GetDirectoryName(item.Path)!, "Disabled");
                Directory.CreateDirectory(disabledDir);
                File.Move(item.Path, Path.Combine(disabledDir, Path.GetFileName(item.Path)), true);
            }
            else if (enabled && item.Path is not null)
            {
                var disabled = Path.Combine(Path.GetDirectoryName(item.Path)!, "Disabled", Path.GetFileName(item.Path));
                if (File.Exists(disabled))
                {
                    File.Move(disabled, item.Path, true);
                }
            }
        }
        else if (item.Source == "taskScheduler")
        {
            using var ts = new TaskService();
            var task = ts.FindTask(item.Name);
            if (task is not null) task.Enabled = enabled;
        }
    }

    public void Delete(string id)
    {
        var item = GetAll().FirstOrDefault(i => i.Id == id) ?? throw new InvalidOperationException("Startup item not found");
        if (item.Source == "registryCurrentUser" || item.Source == "registryLocalMachine")
        {
            var hive = item.Source == "registryLocalMachine" ? Registry.LocalMachine : Registry.CurrentUser;
            hive.OpenSubKey(RunPath, true)?.DeleteValue(item.Name, false);
            hive.OpenSubKey(ApprovedRunPath, true)?.DeleteValue(item.Name, false);
        }
        else if (item.Path is not null && File.Exists(item.Path))
        {
            File.Delete(item.Path);
        }
        else if (item.Source == "taskScheduler")
        {
            using var ts = new TaskService();
            ts.RootFolder.DeleteTask(item.Name, false);
        }
    }

    public void OpenLocation(string id)
    {
        var item = GetAll().FirstOrDefault(i => i.Id == id);
        if (item?.Path is not null) ProcessRunner.OpenFolder(item.Path);
        else if (item?.Source.StartsWith("registry") == true) ProcessRunner.StartDetached("regedit");
    }

    /// <summary>StartupApproved first byte 2 = enabled; any other leading byte disables the Run entry.</summary>
    public static bool IsApprovedEnabled(byte[]? approvedBytes) =>
        approvedBytes is null || approvedBytes.Length == 0 || approvedBytes[0] == 2;

    public static byte[] EnabledBytes() => [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    public static byte[] DisabledBytes() => [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    private static IEnumerable<StartupItem> FromRegistry(RegistryKey hive, string path, string source)
    {
        using var key = hive.OpenSubKey(path);
        if (key is null) yield break;
        using var approved = hive.OpenSubKey(ApprovedRunPath);
        foreach (var name in key.GetValueNames())
        {
            var command = key.GetValue(name)?.ToString() ?? "";
            var approvedBytes = approved?.GetValue(name) as byte[];
            yield return new StartupItem
            {
                Id = $"{source}:{name}",
                Name = name,
                Command = command,
                Path = ExtractPath(command),
                Source = source,
                SourceLocation = $"{hive.Name}\\{path}",
                Enabled = IsApprovedEnabled(approvedBytes)
            };
        }
    }

    private static IEnumerable<StartupItem> FromFolder(string folder, string source)
    {
        if (!Directory.Exists(folder)) yield break;
        foreach (var file in Directory.GetFiles(folder))
        {
            yield return new StartupItem
            {
                Id = $"{source}:{Path.GetFileName(file)}",
                Name = Path.GetFileNameWithoutExtension(file),
                Command = file,
                Path = file,
                Source = source,
                SourceLocation = folder,
                Enabled = true
            };
        }
    }

    private static IEnumerable<StartupItem> FromTasks()
    {
        using var ts = new TaskService();
        foreach (var task in ts.RootFolder.Tasks.Cast<Task>())
        {
            var logon = task.Definition.Triggers.Any(t => t.TriggerType == TaskTriggerType.Logon);
            if (!logon) continue;
            yield return new StartupItem
            {
                Id = $"taskScheduler:{task.Name}",
                Name = task.Name,
                Command = task.Definition.Actions.ToString() ?? "",
                Source = "taskScheduler",
                SourceLocation = task.Path,
                Enabled = task.Enabled,
                Description = task.Definition.RegistrationInfo.Description
            };
        }
    }

    private static string? ExtractPath(string command)
    {
        var match = System.Text.RegularExpressions.Regex.Match(command, "\"([^\"]+)\"");
        return match.Success ? match.Groups[1].Value : command.Split(' ').FirstOrDefault();
    }
}
