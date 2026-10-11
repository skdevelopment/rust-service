using System.Diagnostics;
using System.Diagnostics.Eventing.Reader;
using RustService.Models;

namespace RustService.Services;

public sealed class EventLogViewer
{
    public IReadOnlyList<EventLogSource> GetSources()
    {
        var names = new[] { "Application", "System", "Security", "Setup" };
        return names.Select(name =>
        {
            try
            {
                using var log = new EventLog(name);
                return new EventLogSource
                {
                    Name = name,
                    DisplayName = name,
                    RecordsCount = log.Entries.Count,
                    LogType = log.LogDisplayName
                };
            }
            catch
            {
                return new EventLogSource { Name = name, DisplayName = name, LogType = "Unavailable" };
            }
        }).ToList();
    }

    public IReadOnlyList<Models.EventLogEntry> Query(string logName, string? level = null, string? source = null, string? keyword = null, int limit = 200)
    {
        var query = $"*[System[TimeCreated[timediff(@SystemTime) <= 2592000000]]]";
        var entries = new List<Models.EventLogEntry>();
        try
        {
            using var reader = new EventLogReader(new EventLogQuery(logName, PathType.LogName, query) { ReverseDirection = true });
            for (var rec = reader.ReadEvent(); rec is not null && entries.Count < limit; rec = reader.ReadEvent())
            {
                using (rec)
                {
                    var mapped = Map(rec);
                    if (level is not null && !level.Equals("All", StringComparison.OrdinalIgnoreCase)
                        && !mapped.LevelDisplay.Equals(level, StringComparison.OrdinalIgnoreCase)
                        && !mapped.Level.Equals(level, StringComparison.OrdinalIgnoreCase))
                    {
                        continue;
                    }

                    if (!string.IsNullOrWhiteSpace(source) &&
                        mapped.Source.IndexOf(source, StringComparison.OrdinalIgnoreCase) < 0 &&
                        mapped.ProviderName.IndexOf(source, StringComparison.OrdinalIgnoreCase) < 0)
                    {
                        continue;
                    }

                    if (!string.IsNullOrWhiteSpace(keyword) &&
                        mapped.Message.IndexOf(keyword, StringComparison.OrdinalIgnoreCase) < 0)
                    {
                        continue;
                    }

                    entries.Add(mapped);
                }
            }
        }
        catch (Exception ex)
        {
                    entries.Add(new Models.EventLogEntry { Message = ex.Message, Level = "error", LevelDisplay = "Error", Source = logName });
        }

        return entries;
    }

    public EventLogStats Stats(string logName)
    {
        var stats = new EventLogStats { LogName = logName };
        foreach (var entry in Query(logName, limit: 2000))
        {
            if (!DateTime.TryParse(entry.TimeCreated, out var time)) continue;
            var age = DateTime.UtcNow - time.ToUniversalTime();
            var isError = entry.Level is "error" or "critical";
            var isWarn = entry.Level == "warning";
            var isCrit = entry.Level == "critical";
            if (age.TotalHours <= 24)
            {
                if (isError) stats.Errors24h++;
                if (isWarn) stats.Warnings24h++;
                if (isCrit) stats.Critical24h++;
            }

            if (age.TotalDays <= 7)
            {
                if (isError) stats.Errors7d++;
                if (isWarn) stats.Warnings7d++;
            }

            if (age.TotalDays <= 30)
            {
                if (isError) stats.Errors30d++;
                if (isWarn) stats.Warnings30d++;
            }
        }

        return stats;
    }

    private static Models.EventLogEntry Map(EventRecord rec)
    {
        var level = rec.LevelDisplayName?.ToLowerInvariant() switch
        {
            "critical" => "critical",
            "error" => "error",
            "warning" => "warning",
            "information" or "info" => "information",
            "verbose" => "verbose",
            _ => "unknown"
        };

        string message;
        try { message = rec.FormatDescription() ?? ""; }
        catch { message = rec.TaskDisplayName ?? ""; }

        return new Models.EventLogEntry
        {
            Id = rec.Id,
            RecordId = rec.RecordId ?? 0,
            TimeCreated = rec.TimeCreated?.ToUniversalTime().ToString("O") ?? "",
            Level = level,
            LevelDisplay = rec.LevelDisplayName ?? level,
            Source = rec.LogName ?? "",
            ProviderName = rec.ProviderName ?? "",
            Message = message,
            TaskCategory = rec.TaskDisplayName,
            Computer = rec.MachineName ?? "",
            User = rec.UserId?.Value
        };
    }
}
