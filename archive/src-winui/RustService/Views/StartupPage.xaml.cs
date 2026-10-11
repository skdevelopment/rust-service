using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;

namespace RustService.Views;

public sealed partial class StartupPage : Page
{
    private List<StartupItem> shown = [];

    public StartupPage()
    {
        InitializeComponent();
        Reload(resetSources: true);
    }

    private void Search_Changed(object sender, TextChangedEventArgs e) => Reload(false);
    private void Source_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (List is null) return;
        Reload(false);
    }

    private void Refresh_Click(object sender, RoutedEventArgs e) => Reload(true);

    private void Reload(bool resetSources)
    {
        var all = Commands.GetStartupItems();
        if (resetSources || SourceBox.Items.Count == 0)
        {
            var sources = all.Select(i => i.Source).Where(s => !string.IsNullOrWhiteSpace(s)).Distinct(StringComparer.OrdinalIgnoreCase).OrderBy(s => s).ToList();
            sources.Insert(0, "All");
            var keep = SourceBox.SelectedItem?.ToString();
            SourceBox.ItemsSource = sources;
            SourceBox.SelectedItem = keep is not null && sources.Contains(keep) ? keep : "All";
        }

        IEnumerable<StartupItem> items = all;
        var q = SearchBox.Text?.Trim() ?? "";
        if (q.Length > 0)
        {
            items = items.Where(i =>
                i.Name.Contains(q, StringComparison.OrdinalIgnoreCase) ||
                (i.Command ?? "").Contains(q, StringComparison.OrdinalIgnoreCase) ||
                (i.Source ?? "").Contains(q, StringComparison.OrdinalIgnoreCase));
        }

        var source = SourceBox.SelectedItem?.ToString();
        if (!string.IsNullOrWhiteSpace(source) && source != "All")
        {
            items = items.Where(i => i.Source.Equals(source, StringComparison.OrdinalIgnoreCase));
        }

        shown = items.OrderBy(i => i.Name).ToList();
        List.ItemsSource = shown.Select(i => new CardRow
        {
            Title = i.Name,
            Subtitle = i.Command,
            Meta = i.Source,
            Badge = i.Enabled ? "On" : "Off"
        }).ToList();
    }

    private StartupItem? Selected() =>
        List.SelectedIndex >= 0 && List.SelectedIndex < shown.Count ? shown[List.SelectedIndex] : null;

    private void Toggle_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        Commands.ToggleStartupItem(item.Id, !item.Enabled);
        Reload(false);
    }

    private void Delete_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        Commands.DeleteStartupItem(item.Id);
        Reload(true);
    }

    private void Open_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is not null) Commands.OpenStartupItemLocation(item.Id);
    }
}
