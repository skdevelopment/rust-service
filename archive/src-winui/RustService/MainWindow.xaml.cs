using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using RustService.Services;
using RustService.Views;

namespace RustService;

public sealed partial class MainWindow : Window
{
    public static MainWindow? Shell { get; private set; }
    private bool chromeReady;

    public MainWindow()
    {
        InitializeComponent();
        Shell = this;
        Title = "RustService";
        Activated += MainWindow_Activated;
    }

    private void MainWindow_Activated(object sender, WindowActivatedEventArgs args)
    {
        if (chromeReady) return;
        chromeReady = true;
        try
        {
            ExtendsContentIntoTitleBar = true;
            SetTitleBar(AppTitleBar);
            SystemBackdrop ??= new MicaBackdrop();
            var bar = AppWindow?.TitleBar;
            if (bar is not null)
            {
                bar.ExtendsContentIntoTitleBar = true;
                bar.ButtonBackgroundColor = Microsoft.UI.Colors.Transparent;
                bar.ButtonInactiveBackgroundColor = Microsoft.UI.Colors.Transparent;
            }
        }
        catch
        {
            // Caption customization is optional.
        }
    }

    public void ShowStatus(string title, string message, InfoBarSeverity severity = InfoBarSeverity.Informational)
    {
        StatusBar.Title = title;
        StatusBar.Message = message;
        StatusBar.Severity = severity;
        StatusBar.IsOpen = true;
    }

    private void TitleBar_PaneToggleRequested(TitleBar sender, object args) =>
        Nav.IsPaneOpen = !Nav.IsPaneOpen;

    private void Nav_Loaded(object sender, RoutedEventArgs e)
    {
        if (AppServices.IsInitialized)
        {
            foreach (var tab in AppServices.Settings.Current.TechnicianTabs.Tabs)
            {
                Nav.MenuItems.Add(new NavigationViewItem
                {
                    Content = tab.Name,
                    Tag = $"tab:{tab.Url}",
                    Icon = new SymbolIcon(Symbol.Globe)
                });
            }
        }

        Nav.SelectedItem = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault();
    }

    public Type PageTypeFor(string tag) => tag switch
    {
        "service" => typeof(ServicePage),
        "agent" => typeof(AgentPage),
        "system-info" => typeof(SystemInfoPage),
        "component-test" => typeof(ComponentTestPage),
        "shortcuts" => typeof(ShortcutsPage),
        "programs" => typeof(ProgramsPage),
        "scripts" => typeof(ScriptsPage),
        "network-diagnostics" => typeof(NetworkPage),
        "startup-manager" => typeof(StartupPage),
        "event-log" => typeof(EventLogPage),
        "bluescreen" => typeof(BluescreenPage),
        "reports" => typeof(ReportsPage),
        "settings" => typeof(SettingsPage),
        _ => typeof(ServicePage)
    };

    public bool NavigateToTag(string tag)
    {
        try
        {
            if (tag.StartsWith("tab:", StringComparison.Ordinal))
            {
                ContentFrame.Navigate(typeof(TechnicianTabPage), tag[4..]);
                return true;
            }

            ContentFrame.Navigate(PageTypeFor(tag));
            return true;
        }
        catch (Exception ex)
        {
            App.Log($"tab-fail {tag} {ex}");
            ShowStatus("Navigation failed", ex.Message, InfoBarSeverity.Error);
            ContentFrame.Content = new TextBlock { Text = ex.ToString(), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(16) };
            return false;
        }
    }

    public IReadOnlyList<string> VisitAllTabs()
    {
        var lines = new List<string>();
        foreach (var (tag, name) in ProductTabs.All)
        {
            var ok = NavigateToTag(tag);
            var line = ok ? $"OK {name} ({tag})" : $"FAIL {name} ({tag})";
            lines.Add(line);
            App.Log("tab-loop " + line);
        }

        var path = Path.Combine(AppContext.BaseDirectory, "data", "logs", "tab-loop.log");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllLines(path, lines);
        ShowStatus("Tab loop", lines.All(l => l.StartsWith("OK")) ? "All 13 tabs loaded" : "One or more tabs failed",
            lines.All(l => l.StartsWith("OK")) ? InfoBarSeverity.Success : InfoBarSeverity.Error);
        return lines;
    }

    private void Nav_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        ContentFrame.ContentTransitions.Clear();
        if (!AppServices.IsInitialized || AppServices.Settings.Current.Appearance.EnableAnimations)
        {
            ContentFrame.ContentTransitions.Add(new NavigationThemeTransition());
        }

        if (args.SelectedItem is not NavigationViewItem item || item.Tag is not string tag) return;
        NavigateToTag(tag);
    }
}
