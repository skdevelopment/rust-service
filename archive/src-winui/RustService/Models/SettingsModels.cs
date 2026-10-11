namespace RustService.Models;

public sealed class AppSettings
{
    public string Version { get; set; } = "0.7.0";
    public AppearanceSettings Appearance { get; set; } = new();
    public DataSettings Data { get; set; } = new();
    public ReportsSettings Reports { get; set; } = new();
    public BusinessSettings Business { get; set; } = new();
    public ProgramsSettings Programs { get; set; } = new();
    public TechnicianTabsSettings TechnicianTabs { get; set; } = new();
    public PresetsSettings Presets { get; set; } = new();
    public AgentSettings Agent { get; set; } = new();
}

public sealed class AppearanceSettings
{
    public string Theme { get; set; } = "system";
    public string ColorScheme { get; set; } = "forge";
    public bool EnableAnimations { get; set; } = true;
}

public sealed class ColorSchemeInfo
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public string Primary { get; set; } = "";
}

public static class ColorSchemes
{
    public static readonly IReadOnlyList<ColorSchemeInfo> All =
    [
        new() { Id = "forge", Name = "Forge", Description = "Warm ember/copper workshop aesthetic", Primary = "#d97a2b" },
        new() { Id = "cobalt", Name = "Cobalt", Description = "Rich blue, professional diagnostic feel", Primary = "#3b7dd8" },
        new() { Id = "circuit", Name = "Circuit", Description = "Electric teal diagnostic scanner", Primary = "#22b8a0" },
        new() { Id = "oxide", Name = "Oxide", Description = "Burnt terracotta on warm grays", Primary = "#c05a30" },
        new() { Id = "neon-workshop", Name = "Neon Workshop", Description = "Vivid green workbench LEDs", Primary = "#3ddc6a" },
        new() { Id = "techbay", Name = "Techbay", Description = "Coral red accent theme", Primary = "#ce5d5b" },
        new() { Id = "autoservice", Name = "AutoService", Description = "Original AutoService-inspired dark UI", Primary = "#4f8cff" },
        new() { Id = "amber-minimal", Name = "Amber Minimal", Description = "Warm amber accent theme", Primary = "#f59e0b" },
        new() { Id = "amethyst-haze", Name = "Amethyst Haze", Description = "Soft purple tones", Primary = "#8a79ab" },
        new() { Id = "catppuccin", Name = "Catppuccin", Description = "Pastel color palette", Primary = "#8839ef" },
        new() { Id = "supabase", Name = "Supabase", Description = "Green developer theme", Primary = "#72e3ad" },
        new() { Id = "nature", Name = "Nature", Description = "Earthy green tones", Primary = "#2e7d32" },
        new() { Id = "cyberpunk", Name = "Cyberpunk", Description = "Neon pink and cyan", Primary = "#ff00c8" },
        new() { Id = "claude", Name = "Claude", Description = "Anthropic brand-inspired professional theme", Primary = "#d97757" },
        new() { Id = "default", Name = "Default", Description = "Fluent system accent", Primary = "#0078d4" },
    ];
}

public sealed class DataSettings
{
    public string LogLevel { get; set; } = "info";
}

public sealed class ReportsSettings
{
    public bool AutoSaveReports { get; set; } = true;
    public uint ReportRetentionDays { get; set; }
    public bool IncludeLogsInReport { get; set; } = true;
}

public sealed class BusinessSettings
{
    public bool Enabled { get; set; }
    public string Name { get; set; } = "";
    public string? LogoPath { get; set; }
    public string Address { get; set; } = "";
    public string Phone { get; set; } = "";
    public string Email { get; set; } = "";
    public string Website { get; set; } = "";
    public string Tfn { get; set; } = "";
    public string Abn { get; set; } = "";
    public List<string> Technicians { get; set; } = [];
}

public sealed class ProgramsSettings
{
    public Dictionary<string, string> Overrides { get; set; } = [];
}

public sealed class TechnicianTab
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Url { get; set; } = "";
    public string? Icon { get; set; }
}

public sealed class TechnicianTabsSettings
{
    public List<TechnicianTab> Tabs { get; set; } = [];
    public bool UseFavicons { get; set; } = true;
}

public sealed class PresetsSettings
{
    public List<ServicePreset> CustomPresets { get; set; } = [];
}
