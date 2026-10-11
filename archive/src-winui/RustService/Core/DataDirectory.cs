namespace RustService.Core;

/// <summary>
/// Portable data folder next to the executable (unpackaged).
/// Layout: programs/, reports/, scripts/, logs/, settings.json, agent.db.
/// </summary>
public static class DataDirectory
{
    public static string Root { get; private set; } = "";

    public static void Ensure(string? root = null)
    {
        Root = string.IsNullOrWhiteSpace(root)
            ? Path.Combine(AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar), "data")
            : Path.GetFullPath(root);

        foreach (var sub in new[] { "programs", "logs", "reports", "scripts", "agent", "business", "instruments" })
        {
            Directory.CreateDirectory(Path.Combine(Root, sub));
        }

        Directory.CreateDirectory(Path.Combine(Root, "programs", "icons"));
        Directory.CreateDirectory(Path.Combine(Root, "agent", "files", "uploaded"));
        Directory.CreateDirectory(Path.Combine(Root, "agent", "files", "generated"));
        Directory.CreateDirectory(Path.Combine(Root, "agent", "files", "thumbnails"));

        var settingsPath = Path.Combine(Root, "settings.json");
        if (!File.Exists(settingsPath))
        {
            File.WriteAllText(settingsPath,
                """
                {
                  "version": "0.7.0",
                  "appearance": {
                    "theme": "system",
                    "colorScheme": "forge",
                    "enableAnimations": true
                  }
                }
                """);
        }

        var agentDb = Path.Combine(Root, "agent.db");
        var legacy = Path.Combine(Root, "agent", "memory.db");
        if (!File.Exists(agentDb) || new FileInfo(agentDb).Length == 0)
        {
            if (File.Exists(legacy) && new FileInfo(legacy).Length > 0)
            {
                File.Copy(legacy, agentDb, overwrite: true);
            }
            else if (!File.Exists(agentDb))
            {
                using var fs = new FileStream(agentDb, FileMode.CreateNew, FileAccess.Write, FileShare.Read);
            }
        }
    }

    public static string PathTo(params string[] parts) =>
        Path.Combine(new[] { Root }.Concat(parts).ToArray());
}
