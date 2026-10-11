using System.Text.Json;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;

namespace RustService.Views;

public sealed partial class SettingsPage : Page
{
    public SettingsPage()
    {
        InitializeComponent();
        var s = Commands.GetSettings();
        ThemeBox.SelectedIndex = s.Appearance.Theme switch { "light" => 1, "dark" => 2, _ => 0 };
        SchemeBox.ItemsSource = ColorSchemes.All.Select(c => $"{c.Name} ({c.Id})").ToList();
        var schemeIndex = ColorSchemes.All.ToList().FindIndex(c => c.Id == s.Appearance.ColorScheme);
        SchemeBox.SelectedIndex = schemeIndex >= 0 ? schemeIndex : 0;
        AnimSwitch.IsOn = s.Appearance.EnableAnimations;
        BizName.Text = s.Business.Name;
        BizPhone.Text = s.Business.Phone;
        BizEmail.Text = s.Business.Email;
        BizAddress.Text = s.Business.Address;
        BizWebsite.Text = s.Business.Website;
        BizAbn.Text = s.Business.Abn;
        LogoPathBox.Text = Commands.GetBusinessLogo() ?? "";
        ProviderBox.ItemsSource = Enum.GetNames<AgentProvider>();
        ProviderBox.SelectedItem = s.Agent.Provider.ToString();
        ProviderBox.SelectionChanged += ProviderBox_SelectionChanged;
        ModelBox.Text = s.Agent.Model;
        BaseUrlBox.Text = s.Agent.BaseUrl ?? "";
        ApiKeyBox.Password = s.Agent.ApiKeys.Get(s.Agent.Provider) ?? "";
        PromptBox.Text = s.Agent.SystemPrompt ?? "";
        SearchProviderBox.SelectedIndex = s.Agent.SearchProvider switch { SearchProvider.Tavily => 1, SearchProvider.Searxng => 2, _ => 0 };
        TavilyBox.Password = s.Agent.TavilyApiKey ?? "";
        SearxBox.Text = s.Agent.SearxngUrl ?? "";
        ApprovalBox.SelectedIndex = s.Agent.ApprovalMode switch { ApprovalMode.Whitelist => 1, ApprovalMode.Yolo => 2, _ => 0 };
        WhitelistBox.Text = string.Join("\r\n", s.Agent.WhitelistedCommands);
        McpSwitch.IsOn = s.Agent.McpServerEnabled;
        McpPort.Value = s.Agent.McpPort;
        McpKey.Text = s.Agent.McpApiKey ?? "";
        TabsBox.Text = string.Join("\r\n", s.TechnicianTabs.Tabs.Select(t => $"{t.Name}|{t.Url}"));
        LogBox.SelectedIndex = s.Data.LogLevel switch { "error" => 0, "warn" => 1, "debug" => 3, _ => 2 };
        ReloadRequired();
        _ = Commands.GetRequiredPrograms();
        _ = Commands.GetAgentSettings();
    }

    private void ReloadRequired() =>
        RequiredList.ItemsSource = Commands.GetRequiredProgramsStatus()
            .Select(p => new CardRow
            {
                Title = p.Definition.Name,
                Subtitle = p.Definition.Description,
                Meta = p.Path ?? p.Definition.Id,
                Badge = p.Found ? "Found" : "Missing"
            }).ToList();

    private void Save_Click(object sender, RoutedEventArgs e)
    {
        void Set(string key, string json)
        {
            using var doc = JsonDocument.Parse(json);
            Commands.UpdateSetting(key, doc.RootElement.Clone());
        }

        Set("appearance.theme", JsonSerializer.Serialize((ThemeBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "system"));
        var schemeId = SchemeBox.SelectedIndex >= 0 && SchemeBox.SelectedIndex < ColorSchemes.All.Count
            ? ColorSchemes.All[SchemeBox.SelectedIndex].Id
            : "forge";
        Set("appearance.colorScheme", JsonSerializer.Serialize(schemeId));
        Set("appearance.enableAnimations", AnimSwitch.IsOn ? "true" : "false");
        Set("business.name", JsonSerializer.Serialize(BizName.Text));
        Set("business.phone", JsonSerializer.Serialize(BizPhone.Text));
        Set("business.email", JsonSerializer.Serialize(BizEmail.Text));
        Set("business.address", JsonSerializer.Serialize(BizAddress.Text));
        Set("business.website", JsonSerializer.Serialize(BizWebsite.Text));
        Set("business.abn", JsonSerializer.Serialize(BizAbn.Text));
        var s = Commands.GetSettings();
        if (Enum.TryParse<AgentProvider>(ProviderBox.SelectedItem?.ToString(), out var provider))
        {
            s.Agent.Provider = provider;
        }

        s.Agent.Model = ModelBox.Text;
        s.Agent.BaseUrl = string.IsNullOrWhiteSpace(BaseUrlBox.Text) ? null : BaseUrlBox.Text;
        s.Agent.SystemPrompt = string.IsNullOrWhiteSpace(PromptBox.Text) ? null : PromptBox.Text;
        s.Agent.SearchProvider = SearchProviderBox.SelectedIndex switch
        {
            1 => SearchProvider.Tavily,
            2 => SearchProvider.Searxng,
            _ => SearchProvider.None
        };
        s.Agent.TavilyApiKey = string.IsNullOrWhiteSpace(TavilyBox.Password) ? null : TavilyBox.Password;
        s.Agent.SearxngUrl = string.IsNullOrWhiteSpace(SearxBox.Text) ? null : SearxBox.Text;
        s.Agent.ApiKeys.Set(s.Agent.Provider, string.IsNullOrWhiteSpace(ApiKeyBox.Password) ? null : ApiKeyBox.Password);
        s.Agent.ApprovalMode = ApprovalBox.SelectedIndex switch
        {
            1 => ApprovalMode.Whitelist,
            2 => ApprovalMode.Yolo,
            _ => ApprovalMode.Always
        };
        s.Agent.WhitelistedCommands = WhitelistBox.Text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries)
            .Select(l => l.Trim()).Where(l => l.Length > 0).ToList();
        s.Agent.McpServerEnabled = McpSwitch.IsOn;
        s.Agent.McpPort = (int)McpPort.Value;
        s.Agent.McpApiKey = string.IsNullOrWhiteSpace(McpKey.Text) ? s.Agent.McpApiKey : McpKey.Text;
        s.Data.LogLevel = (LogBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "info";
        s.TechnicianTabs.Tabs = TabsBox.Text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries)
            .Select(line => line.Split('|', 2))
            .Where(p => p.Length == 2 && !string.IsNullOrWhiteSpace(p[0]) && !string.IsNullOrWhiteSpace(p[1]))
            .Select(p => new TechnicianTab { Id = Guid.NewGuid().ToString("N")[..8], Name = p[0].Trim(), Url = p[1].Trim() })
            .ToList();
        Commands.SaveSettings();
        ThemeController.Apply();
        if (s.Agent.McpServerEnabled) AppServices.Mcp.Start();
        else AppServices.Mcp.Stop();
    }

    private void ProviderBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!Enum.TryParse<AgentProvider>(ProviderBox.SelectedItem?.ToString(), out var provider)) return;
        ApiKeyBox.Password = Commands.GetSettings().Agent.ApiKeys.Get(provider) ?? "";
    }

    private void OpenData_Click(object sender, RoutedEventArgs e) => Commands.OpenFolder(Commands.GetDataDir());

    private void SaveLogo_Click(object sender, RoutedEventArgs e)
    {
        if (!File.Exists(LogoPathBox.Text)) return;
        Commands.SaveBusinessLogo(File.ReadAllBytes(LogoPathBox.Text), Path.GetFileName(LogoPathBox.Text));
        LogoPathBox.Text = Commands.GetBusinessLogo() ?? "";
    }

    private void Override_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(OverrideId.Text)) return;
        Commands.SetProgramPathOverride(OverrideId.Text, OverridePath.Text);
        _ = Commands.GetProgramExePath(OverrideId.Text);
        _ = Commands.ValidateRequiredPrograms([OverrideId.Text]);
        _ = Commands.GetRequiredProgramsByIds([OverrideId.Text]);
        ReloadRequired();
    }

    private void ClearOverride_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(OverrideId.Text)) return;
        Commands.SetProgramPathOverride(OverrideId.Text, null);
        ReloadRequired();
    }
}
