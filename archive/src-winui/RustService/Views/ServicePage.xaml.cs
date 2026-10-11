using System.Text.Json;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Models;
using RustService.Services;

namespace RustService.Views;

public sealed partial class ServicePage : Page
{
    private List<ServiceQueueItem> queue = [];
    private ServiceReport? lastReport;

    public ServicePage()
    {
        InitializeComponent();
        ReloadPresets();
        AppServices.Runner.LogReceived += (_, line) => DispatcherQueue.TryEnqueue(() => LogBox.Text += line + "\r\n");
        AppServices.Runner.StateChanged += state => DispatcherQueue.TryEnqueue(() =>
        {
            var report = state.CurrentReport;
            if (report is null)
            {
                StatusText.Text = "Idle";
                return;
            }

            StatusText.Text = $"{report.Status} — {report.Results.Count}/{report.Queue.Count(q => q.Enabled)} finished (parallel={report.ParallelMode})";
            RunProgress.Maximum = Math.Max(1, report.Queue.Count(q => q.Enabled));
            RunProgress.Value = report.Results.Count;
            if (report.Status is ServiceRunStatus.Completed or ServiceRunStatus.Failed or ServiceRunStatus.Cancelled)
            {
                lastReport = report;
                ShowResults(report);
            }
        });
        _ = Commands.GetServiceDefinitions();
        _ = Commands.GetServiceRunState();
        _ = Commands.GetBuiltinPresetIds();
        _ = Commands.ListUsbDrives();
        _ = Commands.GetPcFingerprint();
        _ = Commands.GetServiceTimeMetrics();
        _ = Commands.GetServiceAverages();
        _ = Commands.GetPresetAverages();
        Commands.FlushServiceMetrics();
    }

    private void ReloadPresets()
    {
        var presets = Commands.GetServicePresets();
        PresetList.ItemsSource = presets.Select(p => new CardRow
        {
            Title = p.Name,
            Subtitle = p.Description,
            Meta = $"{p.Services.Count} services · {p.Color}",
            Badge = Commands.GetBuiltinPresetIds().Contains(p.Id) ? "Built-in" : "Custom"
        }).ToList();
        if (presets.Count > 0) PresetList.SelectedIndex = 0;
    }

    private void PresetList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        var presets = Commands.GetServicePresets();
        if (PresetList.SelectedIndex < 0 || PresetList.SelectedIndex >= presets.Count) return;
        var preset = presets[PresetList.SelectedIndex];
        _ = Commands.GetServiceDefinitions();
        queue = ServiceQueueBuilder.FromPreset(preset);
        RefreshQueue();
        _ = Commands.ValidateServiceRequirements(queue.Where(q => q.Enabled).Select(q => q.ServiceId));
    }

    private void RefreshQueue()
    {
        QueueList.ItemsSource = queue.Select(q =>
        {
            var def = ServiceCatalog.Get(q.ServiceId);
            var est = Commands.GetEstimatedTime(q.ServiceId);
            var opts = q.Options.ValueKind is JsonValueKind.Object or JsonValueKind.Array ? q.Options.GetRawText() : "{}";
            return new CardRow
            {
                Title = def?.Name ?? q.ServiceId,
                Subtitle = $"~{est / 1000}s  {string.Join(", ", def?.ExclusiveResources ?? [])}",
                Meta = opts,
                Badge = q.Enabled ? "On" : "Off"
            };
        }).ToList();
    }

    private void ToggleEnabled_Click(object sender, RoutedEventArgs e)
    {
        if (QueueList.SelectedIndex is >= 0 and var i && i < queue.Count)
        {
            queue[i].Enabled = !queue[i].Enabled;
            RefreshQueue();
        }
    }

    private void MoveUp_Click(object sender, RoutedEventArgs e)
    {
        var i = QueueList.SelectedIndex;
        if (i <= 0) return;
        (queue[i - 1], queue[i]) = (queue[i], queue[i - 1]);
        for (var n = 0; n < queue.Count; n++) queue[n].Order = n;
        RefreshQueue();
        QueueList.SelectedIndex = i - 1;
    }

    private void MoveDown_Click(object sender, RoutedEventArgs e)
    {
        var i = QueueList.SelectedIndex;
        if (i < 0 || i >= queue.Count - 1) return;
        (queue[i + 1], queue[i]) = (queue[i], queue[i + 1]);
        for (var n = 0; n < queue.Count; n++) queue[n].Order = n;
        RefreshQueue();
        QueueList.SelectedIndex = i + 1;
    }

    private async void Configure_Click(object sender, RoutedEventArgs e)
    {
        var i = QueueList.SelectedIndex;
        if (i < 0 || i >= queue.Count) return;
        var box = new TextBox
        {
            Text = queue[i].Options.ValueKind == JsonValueKind.Undefined ? "{}" : queue[i].Options.GetRawText(),
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            Height = 180,
            FontFamily = new Microsoft.UI.Xaml.Media.FontFamily("Consolas")
        };
        var dialog = new ContentDialog
        {
            Title = $"Options — {queue[i].ServiceId}",
            Content = box,
            PrimaryButtonText = "Save",
            CloseButtonText = "Cancel",
            XamlRoot = XamlRoot
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
        try
        {
            queue[i].Options = JsonDocument.Parse(string.IsNullOrWhiteSpace(box.Text) ? "{}" : box.Text).RootElement.Clone();
            RefreshQueue();
        }
        catch (Exception ex)
        {
            StatusText.Text = "Invalid options JSON: " + ex.Message;
        }
    }

    private async void AddService_Click(object sender, RoutedEventArgs e)
    {
        var catalog = Commands.GetServiceDefinitions();
        var missing = catalog.Where(d => queue.All(q => !q.ServiceId.Equals(d.Id, StringComparison.OrdinalIgnoreCase))).ToList();
        var source = missing.Count > 0 ? missing : catalog.ToList();
        var box = new ComboBox
        {
            ItemsSource = source.Select(d => d.Name).ToList(),
            SelectedIndex = 0,
            Width = 320
        };
        var dialog = new ContentDialog
        {
            Title = "Add service",
            Content = box,
            PrimaryButtonText = "Add",
            CloseButtonText = "Cancel",
            XamlRoot = XamlRoot
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary) return;
        if (box.SelectedIndex < 0 || box.SelectedIndex >= source.Count) return;
        ServiceQueueBuilder.AddService(queue, source[box.SelectedIndex].Id);
        RefreshQueue();
    }

    private void GoQueue_Click(object sender, RoutedEventArgs e) => Steps.SelectedIndex = 1;
    private void GoPresets_Click(object sender, RoutedEventArgs e) => Steps.SelectedIndex = 0;
    private void GoResults_Click(object sender, RoutedEventArgs e) => Steps.SelectedIndex = 3;

    private async void Start_Click(object sender, RoutedEventArgs e)
    {
        var missing = Commands.ValidateServiceRequirements(queue.Where(q => q.Enabled).Select(q => q.ServiceId));
        if (missing.Count > 0)
        {
            StatusText.Text = "Missing required programs: " + string.Join(", ", missing);
        }

        LogBox.Text = "";
        Steps.SelectedIndex = 2;
        try
        {
            lastReport = await Commands.RunServices(queue, ParallelSwitch.IsChecked == true, TechBox.Text, CustomerBox.Text);
            ShowResults(lastReport);
            Steps.SelectedIndex = 3;
        }
        catch (Exception ex)
        {
            StatusText.Text = ex.Message;
        }
    }

    private void Pause_Click(object sender, RoutedEventArgs e) => Commands.PauseServiceRun();
    private void Resume_Click(object sender, RoutedEventArgs e) => Commands.ResumeServiceRun();
    private void Cancel_Click(object sender, RoutedEventArgs e) => Commands.CancelServiceRun();

    private void ShowResults(ServiceReport report)
    {
        var stats = AppServices.Reports.Statistics(report);
        var rows = report.Results.SelectMany(r => r.Findings.Select(f => new CardRow
        {
            Title = f.Title,
            Subtitle = f.Description,
            Meta = r.ServiceId,
            Badge = f.Severity.ToString()
        })).ToList();
        if (rows.Count == 0)
        {
            rows.Add(new CardRow { Title = $"Health {stats.HealthScore}", Subtitle = $"passed={stats.Passed} failed={stats.Failed}", Badge = "Summary" });
        }

        FindingsList.ItemsSource = rows;
        PrintoutBox.Text = AppServices.Reports.FormatPrintout(report);
        CustomerPrintBox.Text = AppServices.Reports.FormatCustomerPrint(report);
        SummaryBox.Text = report.AgentSummary ?? "";
    }

    private void RefreshResults_Click(object sender, RoutedEventArgs e)
    {
        if (lastReport is null) return;
        lastReport = Commands.GetServiceReport(lastReport.Id) ?? lastReport;
        ShowResults(lastReport);
        if (lastReport is not null) _ = Commands.GetReportStatistics(lastReport.Id);
    }

    private void Pdf_Click(object sender, RoutedEventArgs e)
    {
        if (lastReport is null) return;
        var path = Commands.GenerateReportPdf(lastReport.Id);
        Commands.OpenFolder(path);
    }

    private void SaveSummary_Click(object sender, RoutedEventArgs e)
    {
        if (lastReport is null) return;
        Commands.SetReportSummary(lastReport.Id, SummaryBox.Text);
        lastReport = Commands.GetServiceReport(lastReport.Id);
        if (lastReport is not null) ShowResults(lastReport);
    }

    private void SavePreset_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(CustomPresetName.Text) || queue.Count == 0) return;
        Commands.SaveServicePreset(new ServicePreset
        {
            Id = Guid.NewGuid().ToString("N")[..8],
            Name = CustomPresetName.Text,
            Description = "Custom technician preset",
            Icon = "sliders",
            Color = "gray",
            Services = queue.Select(q => new PresetServiceConfig { ServiceId = q.ServiceId, Enabled = q.Enabled, Options = q.Options }).ToList()
        });
        ReloadPresets();
    }

    private void DeletePreset_Click(object sender, RoutedEventArgs e)
    {
        var presets = Commands.GetServicePresets();
        if (PresetList.SelectedIndex < 0) return;
        var preset = presets[PresetList.SelectedIndex];
        if (Commands.GetBuiltinPresetIds().Contains(preset.Id)) return;
        Commands.DeleteServicePreset(preset.Id);
        ReloadPresets();
    }
}
