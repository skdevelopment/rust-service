using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class ReportStore
{
    public IReadOnlyList<ServiceReport> List()
    {
        var dir = DataDirectory.PathTo("reports");
        if (!Directory.Exists(dir)) return [];
        return Directory.GetFiles(dir, "*.json")
            .Select(path => JsonUtil.Load<ServiceReport>(path, new ServiceReport()))
            .OrderByDescending(r => r.StartedAt)
            .ToList();
    }

    public ServiceReport? Get(string id)
    {
        var path = DataDirectory.PathTo("reports", $"{id}.json");
        return File.Exists(path) ? JsonUtil.Load<ServiceReport>(path, new ServiceReport()) : null;
    }

    public void Save(ServiceReport report)
    {
        JsonUtil.Save(DataDirectory.PathTo("reports", $"{report.Id}.json"), report);
        Prune();
    }

    public bool Delete(string id)
    {
        var path = DataDirectory.PathTo("reports", $"{id}.json");
        if (!File.Exists(path)) return false;
        File.Delete(path);
        return true;
    }

    public void Clear()
    {
        var dir = DataDirectory.PathTo("reports");
        if (!Directory.Exists(dir)) return;
        foreach (var file in Directory.GetFiles(dir, "*.json")) File.Delete(file);
    }

    public ReportStatistics Statistics(ServiceReport report)
    {
        var findings = report.Results.SelectMany(r => r.Findings).ToList();
        var counts = new FindingSeverityCounts
        {
            Info = findings.Count(f => f.Severity == FindingSeverity.Info),
            Success = findings.Count(f => f.Severity == FindingSeverity.Success),
            Warning = findings.Count(f => f.Severity == FindingSeverity.Warning),
            Error = findings.Count(f => f.Severity == FindingSeverity.Error),
            Critical = findings.Count(f => f.Severity == FindingSeverity.Critical)
        };
        var durations = report.Results.Select(r => r.DurationMs).ToList();
        return new ReportStatistics
        {
            TotalServices = report.Results.Count,
            Passed = report.Results.Count(r => r.Success),
            Failed = report.Results.Count(r => !r.Success),
            TotalDurationMs = durations.Sum(),
            AvgDurationMs = durations.Count == 0 ? 0 : (long)durations.Average(),
            SlowestService = report.Results.OrderByDescending(r => r.DurationMs).FirstOrDefault()?.ServiceId,
            FastestService = report.Results.OrderBy(r => r.DurationMs).FirstOrDefault()?.ServiceId,
            FindingsBySeverity = counts,
            TotalFindings = findings.Count,
            HealthScore = report.HealthScore ?? Math.Max(0, 100 - counts.Error * 8 - counts.Critical * 15 - counts.Warning * 3)
        };
    }

    public static IReadOnlyList<UsbDriveInfo> ListUsbDrives() =>
        DriveInfo.GetDrives()
            .Where(d => d.IsReady && d.DriveType == DriveType.Removable)
            .Select(d => new UsbDriveInfo
            {
                Name = string.IsNullOrWhiteSpace(d.VolumeLabel) ? d.Name : d.VolumeLabel,
                Path = d.Name,
                TotalBytes = d.TotalSize,
                FreeBytes = d.AvailableFreeSpace
            }).ToList();

    public string GeneratePrintout(ServiceReport report)
    {
        var path = DataDirectory.PathTo("reports", $"{report.Id}.txt");
        File.WriteAllText(path, FormatPrintout(report));
        return path;
    }

    public string FormatPrintout(ServiceReport report)
    {
        var lines = new List<string>
        {
            "RustService Technical Printout",
            $"Id: {report.Id}",
            $"Started: {report.StartedAt}",
            $"Completed: {report.CompletedAt}",
            $"Status: {report.Status}",
            $"Technician: {report.TechnicianName}",
            $"Customer: {report.CustomerName}",
            $"Parallel: {report.ParallelMode}",
            ""
        };
        foreach (var result in report.Results)
        {
            lines.Add($"## {result.ServiceId} ({(result.Success ? "PASS" : "FAIL")}) {result.DurationMs}ms");
            foreach (var finding in result.Findings)
            {
                lines.Add($"- [{finding.Severity}] {finding.Title}: {finding.Description}");
                if (!string.IsNullOrWhiteSpace(finding.Recommendation))
                {
                    lines.Add($"  Recommendation: {finding.Recommendation}");
                }
            }

            if (!string.IsNullOrWhiteSpace(result.AgentAnalysis))
            {
                lines.Add($"  AI Analysis: {result.AgentAnalysis}");
            }

            lines.Add("");
        }

        return string.Join(Environment.NewLine, lines);
    }

    public string FormatCustomerPrint(ServiceReport report)
    {
        var stats = Statistics(report);
        var biz = AppServices.Settings.Current.Business;
        var lines = new List<string>
        {
            biz.Enabled && !string.IsNullOrWhiteSpace(biz.Name) ? biz.Name : "RustService",
            "Customer service summary",
            $"Customer: {report.CustomerName ?? "—"}",
            $"Technician: {report.TechnicianName ?? "—"}",
            $"Date: {report.StartedAt}",
            $"Overall health: {stats.HealthScore}/100",
            ""
        };
        foreach (var result in report.Results)
        {
            var headline = result.Findings.FirstOrDefault()?.Title ?? (result.Success ? "Completed" : "Needs attention");
            lines.Add($"• {ServiceCatalog.Get(result.ServiceId)?.Name ?? result.ServiceId}: {(result.Success ? "OK" : "Issue")} — {headline}");
        }

        if (!string.IsNullOrWhiteSpace(report.AgentSummary))
        {
            lines.Add("");
            lines.Add(report.AgentSummary);
        }

        return string.Join(Environment.NewLine, lines);
    }

    public string GeneratePdf(string reportId, string? outputPath = null)
    {
        var report = Get(reportId) ?? throw new InvalidOperationException("Report not found");
        var path = outputPath ?? DataDirectory.PathTo("reports", $"{report.Id}.pdf");
        var content = FormatPrintout(report) + Environment.NewLine + FormatCustomerPrint(report);
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, content);
        return path;
    }

    public void EditFinding(string reportId, string serviceId, int findingIndex, string? severity, string? title, string? description, string? recommendation)
    {
        var report = Require(reportId);
        var result = RequireResult(report, serviceId);
        if (findingIndex < 0 || findingIndex >= result.Findings.Count) throw new ArgumentOutOfRangeException(nameof(findingIndex));
        var finding = result.Findings[findingIndex];
        if (severity is not null) finding.Severity = ParseSeverity(severity);
        if (title is not null) finding.Title = title;
        if (description is not null) finding.Description = description;
        if (recommendation is not null) finding.Recommendation = recommendation;
        Save(report);
    }

    public void AddFinding(string reportId, string serviceId, string severity, string title, string description, string? recommendation)
    {
        var report = Require(reportId);
        var result = RequireResult(report, serviceId);
        result.Findings.Add(new ServiceFinding
        {
            Severity = ParseSeverity(severity),
            Title = title,
            Description = description,
            Recommendation = recommendation
        });
        Save(report);
    }

    public void RemoveFinding(string reportId, string serviceId, int findingIndex)
    {
        var report = Require(reportId);
        var result = RequireResult(report, serviceId);
        if (findingIndex < 0 || findingIndex >= result.Findings.Count) throw new ArgumentOutOfRangeException(nameof(findingIndex));
        result.Findings.RemoveAt(findingIndex);
        Save(report);
    }

    public void SetSummary(string reportId, string summary)
    {
        var report = Require(reportId);
        report.AgentSummary = summary;
        Save(report);
    }

    public void SetAnalysis(string reportId, string serviceId, string analysis)
    {
        var report = Require(reportId);
        RequireResult(report, serviceId).AgentAnalysis = analysis;
        Save(report);
    }

    public void SetHealthScore(string reportId, int score)
    {
        var report = Require(reportId);
        report.HealthScore = Math.Clamp(score, 0, 100);
        Save(report);
    }

    private ServiceReport Require(string id) => Get(id) ?? throw new InvalidOperationException("Report not found");

    private static ServiceResult RequireResult(ServiceReport report, string serviceId) =>
        report.Results.FirstOrDefault(r => r.ServiceId == serviceId)
        ?? throw new InvalidOperationException($"Service result not found: {serviceId}");

    private static FindingSeverity ParseSeverity(string severity) => severity.ToLowerInvariant() switch
    {
        "info" => FindingSeverity.Info,
        "success" => FindingSeverity.Success,
        "warning" => FindingSeverity.Warning,
        "error" => FindingSeverity.Error,
        "critical" => FindingSeverity.Critical,
        _ => throw new ArgumentException($"Invalid severity: {severity}")
    };

    private void Prune()
    {
        var days = AppServices.Settings.Current.Reports.ReportRetentionDays;
        if (days == 0) return;
        foreach (var report in List())
        {
            if (DateTime.TryParse(report.StartedAt, out var started)
                && (DateTime.UtcNow - started.ToUniversalTime()).TotalDays > days)
            {
                Delete(report.Id);
            }
        }
    }
}
