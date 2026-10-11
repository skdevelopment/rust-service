using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;

namespace RustService.Views;

public sealed partial class ReportsPage : Page
{
    public ReportsPage()
    {
        InitializeComponent();
        Reload();
    }

    private IReadOnlyList<ServiceReport> Items => Commands.ListServiceReports();

    private void Reload() =>
        List.ItemsSource = Items.Select(r => new CardRow
        {
            Title = r.Status.ToString(),
            Subtitle = $"{r.Results.Count} services",
            Meta = r.StartedAt,
            Badge = r.ParallelMode ? "Parallel" : "Sequential"
        }).ToList();

    private ServiceReport? Selected() =>
        List.SelectedIndex >= 0 && List.SelectedIndex < Items.Count ? Items[List.SelectedIndex] : null;

    private void Refresh_Click(object sender, RoutedEventArgs e) => Reload();

    private void Delete_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is not null) Commands.DeleteReport(item.Id);
        Reload();
    }

    private void Clear_Click(object sender, RoutedEventArgs e)
    {
        Commands.ClearAllReports();
        Reload();
    }

    private void Pdf_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        var path = Commands.GenerateReportPdf(item.Id);
        Commands.OpenFolder(path);
    }

    private void Retrain_Click(object sender, RoutedEventArgs e)
    {
        Commands.RetrainTimeModels();
        Commands.FlushServiceMetrics();
    }

    private void ClearMetrics_Click(object sender, RoutedEventArgs e) => Commands.ClearServiceMetrics();

    private void AddFinding_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null || item.Results.Count == 0) return;
        Commands.AddReportFinding(item.Id, item.Results[0].ServiceId, "info", "Note", "Technician note", null);
        List_SelectionChanged(sender, null!);
    }

    private void RemoveFinding_Click(object sender, RoutedEventArgs e)
    {
        var item = Selected();
        if (item is null || item.Results.Count == 0 || item.Results[0].Findings.Count == 0) return;
        Commands.EditReportFinding(item.Id, item.Results[0].ServiceId, 0, "info", item.Results[0].Findings[0].Title, item.Results[0].Findings[0].Description, item.Results[0].Findings[0].Recommendation);
        Commands.RemoveReportFinding(item.Id, item.Results[0].ServiceId, 0);
        List_SelectionChanged(sender, null!);
    }

    private void List_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        var item = Selected();
        if (item is null) return;
        item = Commands.GetServiceReport(item.Id) ?? item;
        var stats = Commands.GetReportStatistics(item.Id);
        var rows = item.Results.SelectMany(r => r.Findings.Select(f => new CardRow
        {
            Title = f.Title,
            Subtitle = f.Description,
            Meta = r.ServiceId,
            Badge = f.Severity.ToString()
        })).ToList();
        if (rows.Count == 0)
        {
            rows.Add(new CardRow { Title = $"Health {stats.HealthScore}", Subtitle = $"passed={stats.Passed} failed={stats.Failed} findings={stats.TotalFindings}", Badge = "Summary" });
        }

        FindingsList.ItemsSource = rows;
        PrintoutBox.Text = AppServices.Reports.FormatPrintout(item);
        CustomerBox.Text = AppServices.Reports.FormatCustomerPrint(item);
        if (item.Results.Count > 0)
        {
            Commands.SetServiceAnalysis(item.Id, item.Results[0].ServiceId, item.Results[0].AgentAnalysis ?? "");
            Commands.SetReportHealthScore(item.Id, stats.HealthScore);
        }
    }
}
