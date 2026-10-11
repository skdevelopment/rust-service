using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class ServiceTimeSample
{
    public string ServiceId { get; set; } = "";
    public long DurationMs { get; set; }
    public string Timestamp { get; set; } = DateTime.UtcNow.ToString("O");
    public string? PresetId { get; set; }
}

public sealed class ServiceTimeStats
{
    public string ServiceId { get; set; } = "";
    public long AverageMs { get; set; }
    public long MinMs { get; set; }
    public long MaxMs { get; set; }
    public long MedianMs { get; set; }
    public int SampleCount { get; set; }
    public long? EstimatedMs { get; set; }
}

public sealed class PresetTimeStats
{
    public string PresetId { get; set; } = "";
    public long AverageMs { get; set; }
    public int SampleCount { get; set; }
}

public sealed class ServiceTimeMetrics
{
    public string Version { get; set; } = "2.0.0";
    public List<ServiceTimeSample> Samples { get; set; } = [];
}

public sealed class PcFingerprint
{
    public uint PhysicalCores { get; set; }
    public uint LogicalCores { get; set; }
    public double FrequencyGhz { get; set; }
    public double CpuScore { get; set; }
    public double AvailableRamGb { get; set; }
    public double TotalRamGb { get; set; }
    public bool DiskIsSsd { get; set; }
    public bool IsOnAcPower { get; set; }
    public string MachineName { get; set; } = Environment.MachineName;
}

public sealed class TimeTrackingService
{
    private ServiceTimeMetrics metrics = new();
    private bool dirty;

    public ServiceTimeMetrics Get()
    {
        Reload();
        return metrics;
    }

    public void Record(string serviceId, long durationMs, string? presetId = null)
    {
        Reload();
        metrics.Samples.Add(new ServiceTimeSample { ServiceId = serviceId, DurationMs = durationMs, PresetId = presetId });
        dirty = true;
        if (metrics.Samples.Count % 10 == 0) Persist();
    }

    public ServiceTimeStats? Stats(string serviceId)
    {
        Reload();
        var samples = metrics.Samples.Where(s => s.ServiceId == serviceId).Select(s => s.DurationMs).OrderBy(v => v).ToList();
        if (samples.Count == 0) return null;
        return new ServiceTimeStats
        {
            ServiceId = serviceId,
            AverageMs = (long)samples.Average(),
            MinMs = samples.First(),
            MaxMs = samples.Last(),
            MedianMs = samples[samples.Count / 2],
            SampleCount = samples.Count,
            EstimatedMs = (long)samples.Average()
        };
    }

    public long GetEstimatedTime(string serviceId) =>
        Stats(serviceId)?.EstimatedMs ?? (ServiceCatalog.Get(serviceId)?.EstimatedDurationSecs * 1000L ?? 0);

    public IReadOnlyList<ServiceTimeStats> GetServiceAverages() =>
        Get().Samples.Select(s => s.ServiceId).Distinct().Select(id => Stats(id)!).ToList();

    public IReadOnlyList<PresetTimeStats> GetPresetAverages()
    {
        Reload();
        return metrics.Samples
            .Where(s => !string.IsNullOrWhiteSpace(s.PresetId))
            .GroupBy(s => s.PresetId!)
            .Select(g => new PresetTimeStats { PresetId = g.Key, AverageMs = (long)g.Average(s => s.DurationMs), SampleCount = g.Count() })
            .ToList();
    }

    public void Retrain()
    {
        Reload();
        Persist();
    }

    public void Flush()
    {
        if (dirty) Persist();
    }

    public void Clear()
    {
        metrics = new ServiceTimeMetrics();
        Persist();
    }

    public PcFingerprint GetFingerprint()
    {
        var info = AppServices.SystemInfo.Get();
        var physical = info.Cpu.PhysicalCores ?? info.Cpu.LogicalCpus;
        var logical = info.Cpu.LogicalCpus;
        var ghz = info.Cpu.FrequencyMhz / 1000.0;
        var cpuScore = (physical * 1.0 + Math.Max(0, (int)logical - (int)physical) * 0.3) * Math.Max(ghz, 0.1);
        return new PcFingerprint
        {
            PhysicalCores = physical,
            LogicalCores = logical,
            FrequencyGhz = ghz,
            CpuScore = cpuScore,
            AvailableRamGb = info.Memory.AvailableMemory / 1_000_000_000d,
            TotalRamGb = info.Memory.TotalMemory / 1_000_000_000d,
            DiskIsSsd = info.Disks.Any(d => d.DiskType.Contains("SSD", StringComparison.OrdinalIgnoreCase) || d.DiskType.Contains("Fixed", StringComparison.OrdinalIgnoreCase)),
            IsOnAcPower = info.Batteries.Count == 0 || info.Batteries.All(b => !string.Equals(b.State, "Discharging", StringComparison.OrdinalIgnoreCase)),
            MachineName = info.Os.Hostname ?? Environment.MachineName
        };
    }

    private void Reload() => metrics = JsonUtil.Load(DataDirectory.PathTo("service_metrics.json"), new ServiceTimeMetrics());

    private void Persist()
    {
        JsonUtil.Save(DataDirectory.PathTo("service_metrics.json"), metrics);
        dirty = false;
    }
}
