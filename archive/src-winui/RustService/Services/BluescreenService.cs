using System.Text.RegularExpressions;
using RustService.Models;

namespace RustService.Services;

public sealed class BluescreenService
{
    private static readonly Dictionary<string, string> StopCodes = new(StringComparer.OrdinalIgnoreCase)
    {
        ["0x0000000A"] = "IRQL_NOT_LESS_OR_EQUAL",
        ["0x0000001E"] = "KMODE_EXCEPTION_NOT_HANDLED",
        ["0x0000003B"] = "SYSTEM_SERVICE_EXCEPTION",
        ["0x00000050"] = "PAGE_FAULT_IN_NONPAGED_AREA",
        ["0x0000007E"] = "SYSTEM_THREAD_EXCEPTION_NOT_HANDLED",
        ["0x000000D1"] = "DRIVER_IRQL_NOT_LESS_OR_EQUAL",
        ["0x00000116"] = "VIDEO_TDR_FAILURE",
        ["0x00000124"] = "WHEA_UNCORRECTABLE_ERROR",
        ["0x00000133"] = "DPC_WATCHDOG_VIOLATION",
        ["0x00000139"] = "KERNEL_SECURITY_CHECK_FAILURE",
        ["0x0000014C"] = "SYSTEM_PTE_MISUSE",
        ["0x00000154"] = "UNEXPECTED_STORE_EXCEPTION",
        ["0x000000EF"] = "CRITICAL_PROCESS_DIED"
    };

    public IReadOnlyList<BsodEntry> History()
    {
        var dumps = DumpFiles();
        return dumps.Select(ToEntry).ToList();
    }

    public BsodDetails Details(string dumpPath)
    {
        var entry = ToEntry(new FileInfo(dumpPath));
        var name = entry.StopCodeName ?? "Unknown";
        return new BsodDetails
        {
            DumpPath = dumpPath,
            DumpType = Path.GetExtension(dumpPath).Equals(".dmp", StringComparison.OrdinalIgnoreCase) ? "Kernel / minidump" : Path.GetExtension(dumpPath),
            CrashTime = entry.CrashTime,
            StopCode = entry.StopCode,
            StopCodeName = name,
            StopCodeDescription = name.Replace('_', ' '),
            BugCheckCode = entry.BugCheckCode,
            Parameters = entry.Parameters,
            FaultingModule = entry.FaultingModule,
            PossibleCauses = [name.Contains("VIDEO") ? "GPU driver timeout" : "Driver or hardware fault"],
            Recommendations =
            [
                "Update chipset and GPU drivers",
                "Run memory diagnostics and chkdsk",
                "Review Event Viewer around the crash time"
            ]
        };
    }

    public BsodStats Stats()
    {
        var entries = History();
        return new BsodStats
        {
            TotalCrashes = entries.Count,
            CrashesLast7Days = entries.Count(e => AgeDays(e.CrashTime) <= 7),
            CrashesLast30Days = entries.Count(e => AgeDays(e.CrashTime) <= 30),
            MostCommonStopCode = entries.GroupBy(e => e.StopCodeName ?? e.StopCode).OrderByDescending(g => g.Count()).FirstOrDefault()?.Key,
            OldestCrash = entries.OrderBy(e => e.CrashTime).FirstOrDefault()?.CrashTime,
            NewestCrash = entries.OrderByDescending(e => e.CrashTime).FirstOrDefault()?.CrashTime
        };
    }

    public void DeleteDump(string path)
    {
        if (File.Exists(path)) File.Delete(path);
    }

    private static IEnumerable<FileInfo> DumpFiles()
    {
        var dirs = new[]
        {
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows), "Minidump"),
            Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Windows), "MEMORY.DMP")
        };

        foreach (var dir in dirs)
        {
            if (Directory.Exists(dir))
            {
                foreach (var file in new DirectoryInfo(dir).GetFiles("*.dmp"))
                {
                    yield return file;
                }
            }
            else if (File.Exists(dir))
            {
                yield return new FileInfo(dir);
            }
        }
    }

    private static BsodEntry ToEntry(FileInfo file)
    {
        var stop = GuessStopCode(file.Name);
        return new BsodEntry
        {
            Id = file.FullName,
            DumpPath = file.FullName,
            CrashTime = file.LastWriteTimeUtc.ToString("O"),
            StopCode = stop,
            StopCodeName = StopCodes.GetValueOrDefault(stop),
            BugCheckCode = stop,
            FileSizeBytes = file.Length
        };
    }

    private static string GuessStopCode(string name)
    {
        var match = Regex.Match(name, @"0x[0-9A-Fa-f]+");
        return match.Success ? match.Value.ToUpperInvariant() : "UNKNOWN";
    }

    private static double AgeDays(string iso) =>
        DateTime.TryParse(iso, out var dt) ? (DateTime.UtcNow - dt.ToUniversalTime()).TotalDays : 999;
}
