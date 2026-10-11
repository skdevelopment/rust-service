namespace RustService.Services;

/// <summary>Canonical product tab tags/names used by the WinUI NavigationView.</summary>
public static class ProductTabs
{
    public static readonly (string Tag, string Name)[] All =
    [
        ("service", "Service"),
        ("agent", "Agent"),
        ("system-info", "System Info"),
        ("component-test", "Component Test"),
        ("shortcuts", "Shortcuts"),
        ("programs", "Programs"),
        ("scripts", "Scripts"),
        ("network-diagnostics", "Network Diagnostics"),
        ("startup-manager", "Startup Manager"),
        ("event-log", "Event Log"),
        ("bluescreen", "Bluescreen"),
        ("reports", "Reports"),
        ("settings", "Settings")
    ];

    public static IReadOnlyList<string> Tags => All.Select(t => t.Tag).ToList();
    public static IReadOnlyList<string> Names => All.Select(t => t.Name).ToList();
}
