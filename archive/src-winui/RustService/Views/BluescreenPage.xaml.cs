using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;
using Windows.ApplicationModel.DataTransfer;

namespace RustService.Views;

public sealed partial class BluescreenPage : Page
{
    private List<BsodEntry> shown = [];

    public BluescreenPage()
    {
        InitializeComponent();
        Reload();
    }

    private void Search_Changed(object sender, TextChangedEventArgs e) => Reload();
    private void Sort_Changed(object sender, SelectionChangedEventArgs e)
    {
        if (List is null) return;
        Reload();
    }

    private void Refresh_Click(object sender, RoutedEventArgs e) => Reload();

    private void Reload()
    {
        var stats = Commands.GetBsodStats();
        StatsText.Text = $"Crashes {stats.TotalCrashes}  last7d={stats.CrashesLast7Days}  last30d={stats.CrashesLast30Days}  common={stats.MostCommonStopCode}  module={stats.MostCommonModule}";
        IEnumerable<BsodEntry> items = Commands.GetBsodHistory();
        var q = SearchBox.Text?.Trim() ?? "";
        if (q.Length > 0)
        {
            items = items.Where(i =>
                (i.StopCode ?? "").Contains(q, StringComparison.OrdinalIgnoreCase) ||
                (i.StopCodeName ?? "").Contains(q, StringComparison.OrdinalIgnoreCase) ||
                (i.FaultingModule ?? "").Contains(q, StringComparison.OrdinalIgnoreCase) ||
                (i.DumpPath ?? "").Contains(q, StringComparison.OrdinalIgnoreCase));
        }

        shown = ((SortBox.SelectedItem as ComboBoxItem)?.Tag?.ToString() switch
        {
            "oldest" => items.OrderBy(i => i.CrashTime),
            "code" => items.OrderBy(i => i.StopCode),
            _ => items.OrderByDescending(i => i.CrashTime)
        }).ToList();

        List.ItemsSource = shown.Select(i => new CardRow
        {
            Title = $"{i.StopCode} {i.StopCodeName}",
            Subtitle = i.FaultingModule,
            Meta = $"{i.CrashTime}  {(i.FileSizeBytes / 1024.0):0} KB",
            Badge = "Bugcheck"
        }).ToList();
        DetailsBox.Text = "";
    }

    private BsodEntry? Selected() =>
        List.SelectedIndex >= 0 && List.SelectedIndex < shown.Count ? shown[List.SelectedIndex] : null;

    private void List_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        var d = Commands.GetBsodDetails(item.DumpPath);
        DetailsBox.Text =
            $"{d.CrashTime}  {d.DumpType}\r\n{d.StopCode} {d.StopCodeName}\r\n{d.StopCodeDescription}\r\n" +
            $"Module: {d.FaultingModule} {d.FaultingModulePath}\r\nUptime: {d.UptimeBeforeCrash}\r\n" +
            $"Params: {string.Join(" ", d.Parameters)}\r\n\r\nCauses:\r\n{string.Join("\r\n", d.PossibleCauses)}\r\n\r\n" +
            $"Recommendations:\r\n{string.Join("\r\n", d.Recommendations)}\r\n\r\nStack:\r\n{string.Join("\r\n", d.StackTrace.Take(20))}";
    }

    private void Copy_Click(object sender, RoutedEventArgs e)
    {
        var data = new DataPackage();
        data.SetText(string.IsNullOrWhiteSpace(DetailsBox.Text) ? StatsText.Text : DetailsBox.Text);
        Clipboard.SetContent(data);
    }

    private void Delete_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        Commands.DeleteCrashDump(item.DumpPath);
        Reload();
    }
}
