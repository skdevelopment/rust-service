using System.Text.Json;

namespace RustService.Models;

public enum FindingSeverity { Info, Success, Warning, Error, Critical }
public enum ServiceRunStatus { Pending, Running, Paused, Completed, Failed, Cancelled }

public sealed class SelectOption
{
    public string Value { get; set; } = "";
    public string Label { get; set; } = "";
}

public sealed class ServiceOptionSchema
{
    public string Id { get; set; } = "";
    public string Label { get; set; } = "";
    public string OptionType { get; set; } = "string";
    public JsonElement DefaultValue { get; set; }
    public double? Min { get; set; }
    public double? Max { get; set; }
    public List<SelectOption>? Options { get; set; }
    public string? Description { get; set; }
}

public sealed class ServiceDefinition
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public string Category { get; set; } = "";
    public int EstimatedDurationSecs { get; set; }
    public List<string> RequiredPrograms { get; set; } = [];
    public List<ServiceOptionSchema> Options { get; set; } = [];
    public string Icon { get; set; } = "";
    public List<string> ExclusiveResources { get; set; } = [];
    public List<string> Dependencies { get; set; } = [];
}

public sealed class PresetServiceConfig
{
    public string ServiceId { get; set; } = "";
    public bool Enabled { get; set; } = true;
    public JsonElement Options { get; set; }
}

public sealed class ServicePreset
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public List<PresetServiceConfig> Services { get; set; } = [];
    public string Icon { get; set; } = "";
    public string Color { get; set; } = "";
}

public sealed class ServiceQueueItem
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public string ServiceId { get; set; } = "";
    public bool Enabled { get; set; } = true;
    public int Order { get; set; }
    public JsonElement Options { get; set; }
}

public sealed class ServiceFinding
{
    public FindingSeverity Severity { get; set; }
    public string Title { get; set; } = "";
    public string Description { get; set; } = "";
    public string? Recommendation { get; set; }
    public JsonElement? Data { get; set; }
}

public sealed class ServiceResult
{
    public string ServiceId { get; set; } = "";
    public bool Success { get; set; }
    public string? Error { get; set; }
    public long DurationMs { get; set; }
    public List<ServiceFinding> Findings { get; set; } = [];
    public List<string> Logs { get; set; } = [];
    public string? AgentAnalysis { get; set; }
}

public sealed class ServiceReport
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public string StartedAt { get; set; } = DateTime.UtcNow.ToString("O");
    public string? CompletedAt { get; set; }
    public ServiceRunStatus Status { get; set; } = ServiceRunStatus.Pending;
    public long? TotalDurationMs { get; set; }
    public List<ServiceQueueItem> Queue { get; set; } = [];
    public List<ServiceResult> Results { get; set; } = [];
    public int? CurrentServiceIndex { get; set; }
    public List<int> CurrentServiceIndices { get; set; } = [];
    public bool ParallelMode { get; set; }
    public string? TechnicianName { get; set; }
    public string? CustomerName { get; set; }
    public bool AgentInitiated { get; set; }
    public string? AgentSummary { get; set; }
    public int? HealthScore { get; set; }
}

public sealed class ServiceRunState
{
    public bool IsRunning { get; set; }
    public bool IsPaused { get; set; }
    public ServiceReport? CurrentReport { get; set; }
}

public sealed class FindingSeverityCounts
{
    public int Info { get; set; }
    public int Success { get; set; }
    public int Warning { get; set; }
    public int Error { get; set; }
    public int Critical { get; set; }
}

public sealed class ReportStatistics
{
    public int TotalServices { get; set; }
    public int Passed { get; set; }
    public int Failed { get; set; }
    public long TotalDurationMs { get; set; }
    public long AvgDurationMs { get; set; }
    public string? SlowestService { get; set; }
    public string? FastestService { get; set; }
    public FindingSeverityCounts FindingsBySeverity { get; set; } = new();
    public int TotalFindings { get; set; }
    public int HealthScore { get; set; }
}

public sealed class UsbDriveInfo
{
    public string Name { get; set; } = "";
    public string Path { get; set; } = "";
    public long TotalBytes { get; set; }
    public long FreeBytes { get; set; }
}
