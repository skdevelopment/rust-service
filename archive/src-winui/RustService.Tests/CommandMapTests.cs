using System.Text.RegularExpressions;
using RustService.Services;
using Xunit;

namespace RustService.Tests;

public class CommandMapTests
{
    [Fact]
    public void Every_tauri_command_has_a_csharp_counterpart()
    {
        var repo = FindRepo();
        var lib = File.ReadAllText(Path.Combine(repo, "src-tauri", "src", "lib.rs"));
        var names = Regex.Matches(lib, @"commands::([A-Za-z0-9_]+)")
            .Select(m => m.Groups[1].Value)
            .Distinct()
            .ToList();
        Assert.True(names.Count >= 100, $"expected the full invoke_handler list, got {names.Count}");
        foreach (var name in names)
        {
            Assert.Contains(name, Commands.TauriCommandNames);
            var pascal = ToPascal(name);
            Assert.True(typeof(Commands).GetMethods().Any(m => m.Name == pascal),
                $"missing Commands.{pascal} for Tauri command {name}");
        }
    }

    [Fact]
    public void Winui_app_calls_each_command_counterpart()
    {
        var repo = FindRepo();
        var roots = new[]
        {
            Path.Combine(repo, "src-winui", "RustService", "Views"),
            Path.Combine(repo, "src-winui", "RustService", "Services"),
            Path.Combine(repo, "src-winui", "RustService")
        };
        var sources = roots.SelectMany(root => Directory.Exists(root)
                ? Directory.GetFiles(root, "*.cs", SearchOption.AllDirectories)
                : [])
            .Where(path => !path.EndsWith("Commands.cs", StringComparison.OrdinalIgnoreCase))
            .Select(File.ReadAllText)
            .ToList();
        var blob = string.Join("\n", sources);
        foreach (var name in Commands.TauriCommandNames)
        {
            var pascal = ToPascal(name);
            Assert.True(blob.Contains(pascal, StringComparison.Ordinal),
                $"Commands.{pascal} ({name}) is never referenced by the WinUI app / MCP / runner");
        }
    }

    private static string ToPascal(string snake) =>
        string.Concat(snake.Split('_', StringSplitOptions.RemoveEmptyEntries).Select(part =>
            char.ToUpperInvariant(part[0]) + part[1..]));

    internal static string FindRepo()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null)
        {
            if (File.Exists(Path.Combine(dir.FullName, "src-tauri", "src", "lib.rs")))
            {
                return dir.FullName;
            }

            dir = dir.Parent;
        }

        throw new DirectoryNotFoundException("Could not locate rust-service repo root from " + AppContext.BaseDirectory);
    }
}
