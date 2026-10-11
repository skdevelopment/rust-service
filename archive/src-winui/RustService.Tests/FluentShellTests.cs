using RustService.Services;
using Xunit;

namespace RustService.Tests;

public class FluentShellTests
{
    [Fact]
    public void Shell_has_mica_titlebar_nav_icons_and_infobar()
    {
        var repo = CommandMapTests.FindRepo();
        var window = File.ReadAllText(Path.Combine(repo, "src-winui", "RustService", "MainWindow.xaml"));
        Assert.Contains("MicaBackdrop", window);
        Assert.Contains("<TitleBar", window);
        Assert.Contains("InfoBar", window);
        Assert.Contains("NavigationView", window);
        foreach (var tab in new[] { "Service", "Agent", "System Info", "Programs", "Scripts", "Network Diagnostics", "Startup Manager", "Event Log", "Bluescreen", "Reports", "Component Test", "Shortcuts", "Settings" })
        {
            Assert.Contains($"Content=\"{tab}\"", window);
        }

        Assert.Contains("SymbolIcon", window);
        var app = File.ReadAllText(Path.Combine(repo, "src-winui", "RustService", "App.xaml"));
        Assert.Contains("TargetType=\"Page\"", app);
        Assert.Contains("Transparent", app);
        Assert.Contains("CardItemTemplate", app);
    }

    [Fact]
    public void Product_tabs_helper_lists_all_thirteen_names()
    {
        Assert.Equal(13, ProductTabs.All.Length);
        foreach (var name in new[]
                 {
                     "Agent", "Service", "System Info", "Programs", "Scripts", "Network Diagnostics",
                     "Startup Manager", "Event Log", "Bluescreen", "Reports", "Component Test", "Shortcuts", "Settings"
                 })
        {
            Assert.Contains(name, ProductTabs.Names);
        }

        var repo = CommandMapTests.FindRepo();
        var window = File.ReadAllText(Path.Combine(repo, "src-winui", "RustService", "MainWindow.xaml.cs"));
        foreach (var tag in ProductTabs.Tags)
        {
            Assert.Contains($"\"{tag}\"", window);
        }
    }

    [Fact]
    public void Product_listviews_use_item_templates()
    {
        var repo = CommandMapTests.FindRepo();
        var views = Path.Combine(repo, "src-winui", "RustService", "Views");
        foreach (var file in Directory.GetFiles(views, "*.xaml"))
        {
            var name = Path.GetFileName(file);
            if (name.StartsWith("Technician", StringComparison.OrdinalIgnoreCase)) continue;
            var xaml = File.ReadAllText(file);
            var idx = 0;
            while (true)
            {
                var start = xaml.IndexOf("<ListView", idx, StringComparison.Ordinal);
                if (start < 0) break;
                var end = xaml.IndexOf(">", start, StringComparison.Ordinal);
                Assert.True(end > start, name);
                var tag = xaml[start..(end + 1)];
                var selfClose = tag.Contains("/>");
                var hasAttrTemplate = tag.Contains("ItemTemplate");
                var innerEnd = selfClose ? end : xaml.IndexOf("</ListView>", end, StringComparison.Ordinal);
                var block = xaml[start..Math.Max(innerEnd + 10, end)];
                Assert.True(hasAttrTemplate || block.Contains("<ListView.ItemTemplate"),
                    $"{name} has an untemplated ListView: {tag}");
                idx = end + 1;
            }

            idx = 0;
            while (true)
            {
                var start = xaml.IndexOf("<GridView", idx, StringComparison.Ordinal);
                if (start < 0) break;
                var end = xaml.IndexOf(">", start, StringComparison.Ordinal);
                var tag = xaml[start..(end + 1)];
                var selfClose = tag.Contains("/>");
                var innerEnd = selfClose ? end : xaml.IndexOf("</GridView>", end, StringComparison.Ordinal);
                var block = xaml[start..Math.Max(innerEnd + 10, end)];
                Assert.True(tag.Contains("ItemTemplate") || block.Contains("<GridView.ItemTemplate"),
                    $"{name} has an untemplated GridView");
                idx = end + 1;
            }
        }
    }
}
