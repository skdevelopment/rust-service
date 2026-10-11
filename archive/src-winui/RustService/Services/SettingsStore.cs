using System.Text.Json;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class SettingsStore
{
    private readonly object sync = new();
    public AppSettings Current { get; private set; } = new();

    public event Action<AppSettings>? Changed;

    public void Load()
    {
        lock (sync)
        {
            var path = DataDirectory.PathTo("settings.json");
            Current = JsonUtil.Load(path, new AppSettings());
            Current.Version = "0.7.0";
            if (string.IsNullOrWhiteSpace(Current.Appearance.ColorScheme))
            {
                Current.Appearance.ColorScheme = "forge";
            }

            JsonUtil.Save(path, Current);
        }
    }

    public void Save()
    {
        lock (sync)
        {
            JsonUtil.Save(DataDirectory.PathTo("settings.json"), Current);
        }

        Changed?.Invoke(Current);
    }

    public void UpdateSetting(string key, JsonElement value)
    {
        switch (key)
        {
            case "appearance.theme":
                Current.Appearance.Theme = value.GetString() ?? "system";
                break;
            case "appearance.colorScheme":
                Current.Appearance.ColorScheme = value.GetString() ?? "forge";
                break;
            case "appearance.enableAnimations":
                Current.Appearance.EnableAnimations = value.GetBoolean();
                break;
            case "data.logLevel":
                Current.Data.LogLevel = value.GetString() ?? "info";
                break;
            case "reports.autoSaveReports":
                Current.Reports.AutoSaveReports = value.GetBoolean();
                break;
            case "reports.reportRetentionDays":
                Current.Reports.ReportRetentionDays = value.GetUInt32();
                break;
            case "reports.includeLogsInReport":
                Current.Reports.IncludeLogsInReport = value.GetBoolean();
                break;
            case "business.enabled":
                Current.Business.Enabled = value.GetBoolean();
                break;
            case "business.name":
                Current.Business.Name = value.GetString() ?? "";
                break;
            case "business.logoPath":
                Current.Business.LogoPath = value.ValueKind == JsonValueKind.Null ? null : value.GetString();
                break;
            case "business.address":
                Current.Business.Address = value.GetString() ?? "";
                break;
            case "business.phone":
                Current.Business.Phone = value.GetString() ?? "";
                break;
            case "business.email":
                Current.Business.Email = value.GetString() ?? "";
                break;
            case "business.website":
                Current.Business.Website = value.GetString() ?? "";
                break;
            case "business.tfn":
                Current.Business.Tfn = value.GetString() ?? "";
                break;
            case "business.abn":
                Current.Business.Abn = value.GetString() ?? "";
                break;
            case "business.technicians":
                Current.Business.Technicians = JsonSerializer.Deserialize<List<string>>(value.GetRawText(), JsonUtil.Options) ?? [];
                break;
            case "technicianTabs.tabs":
                Current.TechnicianTabs.Tabs = JsonSerializer.Deserialize<List<TechnicianTab>>(value.GetRawText(), JsonUtil.Options) ?? [];
                break;
            case "technicianTabs.useFavicons":
                Current.TechnicianTabs.UseFavicons = value.GetBoolean();
                break;
            case "presets.customPresets":
                Current.Presets.CustomPresets = JsonSerializer.Deserialize<List<ServicePreset>>(value.GetRawText(), JsonUtil.Options) ?? [];
                break;
            case "agent":
                Current.Agent = JsonSerializer.Deserialize<AgentSettings>(value.GetRawText(), JsonUtil.Options) ?? new AgentSettings();
                break;
            case "agent.provider":
                Current.Agent.Provider = Enum.TryParse<AgentProvider>(value.GetString(), true, out var p) ? p : AgentProvider.OpenAI;
                break;
            case "agent.model":
                Current.Agent.Model = value.GetString() ?? Current.Agent.Model;
                break;
            case "agent.approvalMode":
                Current.Agent.ApprovalMode = Enum.TryParse<ApprovalMode>(value.GetString(), true, out var a) ? a : ApprovalMode.Always;
                break;
            case "agent.mcpServerEnabled":
                Current.Agent.McpServerEnabled = value.GetBoolean();
                break;
            case "agent.mcpPort":
                Current.Agent.McpPort = value.GetInt32();
                break;
            case "agent.mcpApiKey":
                Current.Agent.McpApiKey = value.GetString();
                break;
            case "agent.searchProvider":
                Current.Agent.SearchProvider = Enum.TryParse<SearchProvider>(value.GetString(), true, out var sp) ? sp : SearchProvider.None;
                break;
            case "agent.tavilyApiKey":
                Current.Agent.TavilyApiKey = value.GetString();
                break;
            case "agent.searxngUrl":
                Current.Agent.SearxngUrl = value.GetString();
                break;
            case "agent.systemPrompt":
                Current.Agent.SystemPrompt = value.GetString();
                break;
            case "agent.baseUrl":
                Current.Agent.BaseUrl = value.GetString();
                break;
            default:
                throw new ArgumentException($"Unknown setting: {key}");
        }

        Save();
    }

    public string? SaveBusinessLogo(byte[] bytes, string fileName)
    {
        var ext = Path.GetExtension(fileName);
        if (string.IsNullOrWhiteSpace(ext)) ext = ".png";
        var relative = Path.Combine("business", $"logo{ext}");
        File.WriteAllBytes(DataDirectory.PathTo(relative), bytes);
        Current.Business.LogoPath = relative.Replace('\\', '/');
        Save();
        return Current.Business.LogoPath;
    }
}
