using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;
using Windows.ApplicationModel.DataTransfer;

namespace RustService.Views;

public sealed partial class EventLogPage : Page
{
    private IReadOnlyList<EventLogEntry> entries = [];

    public EventLogPage()
    {
        InitializeComponent();
        var sources = Commands.GetEventLogSources();
        LogBox.ItemsSource = sources.Select(s => $"{s.Name} ({s.RecordsCount})").ToList();
        if (sources.Count > 0)
        {
            var system = sources.ToList().FindIndex(s => s.Name.Equals("System", StringComparison.OrdinalIgnoreCase));
            LogBox.SelectedIndex = system >= 0 ? system : 0;
        }

        Query();
    }

    private string LogName
    {
        get
        {
            var sources = Commands.GetEventLogSources();
            return LogBox.SelectedIndex >= 0 && LogBox.SelectedIndex < sources.Count
                ? sources[LogBox.SelectedIndex].Name
                : "System";
        }
    }

    private void Filter_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (List is null) return;
        Query();
    }

    private void Query_Click(object sender, RoutedEventArgs e) => Query();

    private void Query()
    {
        var level = (LevelBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "All";
        var limitText = (LimitBox.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "100";
        _ = int.TryParse(limitText, out var limit);
        if (limit <= 0) limit = 100;
        entries = string.IsNullOrWhiteSpace(KeywordBox.Text)
            ? Commands.GetEventLogs(LogName, level, SourceBox.Text, limit)
            : Commands.SearchEventLogs(LogName, KeywordBox.Text);
        var stats = Commands.GetEventLogStats(LogName);
        StatsText.Text = $"{stats.LogName}: errors 24h={stats.Errors24h} 7d={stats.Errors7d} 30d={stats.Errors30d}  warnings 24h={stats.Warnings24h}  critical 24h={stats.Critical24h}  showing {entries.Count}";
        List.ItemsSource = entries.Select(x => new CardRow
        {
            Title = x.ProviderName,
            Subtitle = Trim(x.Message, 140),
            Meta = x.TimeCreated,
            Badge = x.LevelDisplay
        }).ToList();
        DetailBox.Text = "";
    }

    private void List_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (List.SelectedIndex < 0 || List.SelectedIndex >= entries.Count) return;
        var x = entries[List.SelectedIndex];
        DetailBox.Text = $"{x.TimeCreated}\r\n{x.LevelDisplay}  Event {x.Id}  {x.ProviderName}\r\n{x.Source}\r\n\r\n{x.Message}";
    }

    private void Copy_Click(object sender, RoutedEventArgs e)
    {
        var text = string.IsNullOrWhiteSpace(DetailBox.Text)
            ? string.Join("\r\n", entries.Select(x => $"{x.TimeCreated} [{x.LevelDisplay}] {x.ProviderName}: {x.Message}"))
            : DetailBox.Text;
        var data = new DataPackage();
        data.SetText(text);
        Clipboard.SetContent(data);
    }

    private static string Trim(string value, int max) =>
        value.Length <= max ? value : value[..max] + "…";
}
