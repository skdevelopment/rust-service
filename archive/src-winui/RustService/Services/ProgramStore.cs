using System.Drawing;
using System.Drawing.Imaging;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class ProgramStore
{
    private ProgramConfig config = new();

    public IReadOnlyList<ProgramItem> GetAll()
    {
        Reload();
        return config.Programs;
    }

    public ProgramItem Add(string name, string description, string version, string exePath, bool isCli)
    {
        Reload();
        var item = new ProgramItem
        {
            Name = name,
            Description = description,
            Version = version,
            ExePath = exePath,
            IsCli = isCli
        };
        item.IconPath = ExtractIcon(exePath, item.Id);
        config.Programs.Add(item);
        Persist();
        return item;
    }

    public ProgramItem? Update(ProgramItem updated)
    {
        Reload();
        var index = config.Programs.FindIndex(p => p.Id == updated.Id);
        if (index < 0) return null;
        config.Programs[index] = updated;
        Persist();
        return updated;
    }

    public bool Delete(string id)
    {
        Reload();
        var removed = config.Programs.RemoveAll(p => p.Id == id) > 0;
        if (removed) Persist();
        return removed;
    }

    public ProgramItem? Launch(string id)
    {
        Reload();
        var item = config.Programs.FirstOrDefault(p => p.Id == id);
        if (item is null) return null;
        if (item.IsCli) throw new InvalidOperationException("CLI programs cannot be launched from the GUI.");
        ProcessRunner.StartDetached(item.ExePath);
        item.LaunchCount++;
        item.LastLaunched = DateTime.UtcNow;
        Persist();
        return item;
    }

    public void Reveal(string id)
    {
        Reload();
        var item = config.Programs.FirstOrDefault(p => p.Id == id);
        if (item is null) return;
        ProcessRunner.OpenFolder(item.ExePath);
    }

    public string? GetIconAbsolute(string? relative)
    {
        if (string.IsNullOrWhiteSpace(relative)) return null;
        var path = Path.IsPathRooted(relative) ? relative : DataDirectory.PathTo(relative.Replace('/', Path.DirectorySeparatorChar));
        return File.Exists(path) ? path : null;
    }

    public int ImportLegacy()
    {
        Reload();
        var programsDir = DataDirectory.PathTo("programs");
        if (!Directory.Exists(programsDir)) return 0;
        var added = 0;
        foreach (var exe in Directory.EnumerateFiles(programsDir, "*.exe", SearchOption.AllDirectories))
        {
            if (config.Programs.Any(p => string.Equals(p.ExePath, exe, StringComparison.OrdinalIgnoreCase)))
            {
                continue;
            }

            Add(Path.GetFileNameWithoutExtension(exe), "Imported portable tool", "", exe, false);
            added++;
        }

        return added;
    }

    private void Reload() => config = JsonUtil.Load(DataDirectory.PathTo("programs.json"), new ProgramConfig());

    private void Persist() => JsonUtil.Save(DataDirectory.PathTo("programs.json"), config);

    public string? ExtractIcon(string exePath, string id)
    {
        try
        {
            if (!File.Exists(exePath)) return null;
            using var icon = Icon.ExtractAssociatedIcon(exePath);
            if (icon is null) return null;
            var relative = Path.Combine("programs", "icons", $"{id}.png");
            var dest = DataDirectory.PathTo(relative);
            Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
            using var bmp = icon.ToBitmap();
            bmp.Save(dest, ImageFormat.Png);
            return relative.Replace('\\', '/');
        }
        catch
        {
            return null;
        }
    }
}
