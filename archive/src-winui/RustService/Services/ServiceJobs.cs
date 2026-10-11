using System.Text.Json;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public static class ServiceJobs
{
    public static async Task<ServiceResult> RunAsync(
        string id,
        JsonElement options,
        Action<string> log,
        CancellationToken cancellationToken)
    {
        var started = DateTime.UtcNow;
        try
        {
            var result = id switch
            {
                "restore-point" => await RestorePoint(log, cancellationToken),
                "ping-test" => await Ping(options, log, cancellationToken),
                "disk-space" => DiskSpace(log),
                "winsat" => await External("winsat", "WinSAT", ["diskformal"], log, cancellationToken),
                "kvrt-scan" => await Required("kvrt", ["-accepteula", "-silent"], log, cancellationToken),
                "adwcleaner" => await Required("adwcleaner", ["/eula", "/clean", "/norestart"], log, cancellationToken),
                "stinger" => await Required("stinger", ["--silent"], log, cancellationToken),
                "whynotwin11" => await Required("whynotwin11", ["/export", DataDirectory.PathTo("reports", "whynotwin11.txt")], log, cancellationToken),
                "smartctl" => await Smartctl(log, cancellationToken),
                "speedtest" => await Required("speedtest", ["--accept-license", "--accept-gdpr", "-f", "json"], log, cancellationToken),
                "iperf" => await Iperf(options, log, cancellationToken),
                "bleachbit" => await BleachBit(log, cancellationToken),
                "drivecleanup" => await DriveCleanup(options, log, cancellationToken),
                "heavyload" => await Required("heavyload", [], log, cancellationToken),
                "dism" => await Dism(options, log, cancellationToken),
                "sfc" => await Cmd("sfc", ["/scannow"], log, cancellationToken),
                "chkdsk" => await Chkdsk(options, log, cancellationToken),
                "furmark" => await Furmark(options, log, cancellationToken),
                "windows-update" => await WindowsUpdate(options, log, cancellationToken),
                "energy-report" => await Energy(options, log, cancellationToken),
                "battery-report" => await Battery(log, cancellationToken),
                "driver-audit" => await DriverAudit(options, log, cancellationToken),
                "installed-software" => InstalledSoftware(options, log),
                "network-config" => await NetworkConfig(log, cancellationToken),
                "usb-stability" => await UsbStability(options, log, cancellationToken),
                "startup-optimize" => StartupOptimize(options, log),
                _ => Fail(id, $"Unknown service {id}")
            };

            result.ServiceId = id;
            result.DurationMs = (long)(DateTime.UtcNow - started).TotalMilliseconds;
            return result;
        }
        catch (OperationCanceledException)
        {
            return new ServiceResult
            {
                ServiceId = id,
                Success = false,
                Error = "Cancelled",
                DurationMs = (long)(DateTime.UtcNow - started).TotalMilliseconds,
                Logs = ["Cancelled"]
            };
        }
        catch (Exception ex)
        {
            return new ServiceResult
            {
                ServiceId = id,
                Success = false,
                Error = ex.Message,
                DurationMs = (long)(DateTime.UtcNow - started).TotalMilliseconds,
                Logs = [ex.Message],
                Findings = [new ServiceFinding { Severity = FindingSeverity.Error, Title = "Service failed", Description = ex.Message }]
            };
        }
    }

    private static ServiceResult Fail(string id, string error) =>
        new() { ServiceId = id, Success = false, Error = error, Findings = [new() { Severity = FindingSeverity.Error, Title = error, Description = error }] };

    private static async Task<ServiceResult> RestorePoint(Action<string> log, CancellationToken ct)
    {
        log("Creating restore point...");
        await AppServices.SystemInfo.CreateRestorePointAsync("RustService");
        log("Restore point created.");
        return Ok("restore-point", "Restore point created", "A system restore point was created successfully.");
    }

    private static async Task<ServiceResult> Ping(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var host = options.TryGetProperty("target", out var t) ? t.GetString() ?? "8.8.8.8" : "8.8.8.8";
        var count = options.TryGetProperty("count", out var c) ? c.GetInt32() : 4;
        log($"Pinging {host} x{count}");
        var ping = await AppServices.Network.PingAsync(host, count);
        var success = ping.PacketsReceived > 0;
        return new ServiceResult
        {
            Success = success,
            Error = ping.Error,
            Logs = ping.Replies.Select(r => $"{r.Seq}: {r.Status} {r.TimeMs}ms").ToList(),
            Findings =
            [
                new ServiceFinding
                {
                    Severity = success ? FindingSeverity.Success : FindingSeverity.Error,
                    Title = success ? "Host reachable" : "Host unreachable",
                    Description = $"Loss {ping.PacketLossPercent:0.#}% avg {ping.AvgMs}ms"
                }
            ]
        };
    }

    private static ServiceResult DiskSpace(Action<string> log)
    {
        var findings = new List<ServiceFinding>();
        var logs = new List<string>();
        foreach (var drive in DriveInfo.GetDrives().Where(d => d.IsReady))
        {
            var usedPct = 100.0 - drive.AvailableFreeSpace * 100.0 / drive.TotalSize;
            var line = $"{drive.Name} {usedPct:0.#}% used, {drive.AvailableFreeSpace / 1_000_000_000} GB free";
            log(line);
            logs.Add(line);
            findings.Add(new ServiceFinding
            {
                Severity = usedPct > 90 ? FindingSeverity.Error : usedPct > 80 ? FindingSeverity.Warning : FindingSeverity.Success,
                Title = drive.Name,
                Description = line,
                Recommendation = usedPct > 80 ? "Free disk space or move user data." : null
            });
        }

        return new ServiceResult { Success = true, Logs = logs, Findings = findings };
    }

    private static async Task<ServiceResult> Smartctl(Action<string> log, CancellationToken ct)
    {
        var path = AppServices.RequiredPrograms.GetExePath("smartctl") ?? throw new InvalidOperationException("smartctl not found");
        log("Scanning SMART devices...");
        var scan = await ProcessRunner.RunAsync(path, ["--scan"], cancellationToken: ct);
        log(scan.Combined);
        var findings = new List<ServiceFinding>();
        foreach (var line in scan.Stdout.Split('\n', StringSplitOptions.RemoveEmptyEntries))
        {
            var device = line.Split(' ').FirstOrDefault();
            if (string.IsNullOrWhiteSpace(device)) continue;
            var health = await ProcessRunner.RunAsync(path, ["-H", device], cancellationToken: ct);
            log(health.Combined);
            findings.Add(new ServiceFinding
            {
                Severity = health.Combined.Contains("PASSED", StringComparison.OrdinalIgnoreCase) ? FindingSeverity.Success : FindingSeverity.Warning,
                Title = device,
                Description = health.Stdout.Trim()
            });
        }

        return new ServiceResult { Success = true, Logs = [scan.Combined], Findings = findings };
    }

    private static async Task<ServiceResult> Iperf(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var server = options.TryGetProperty("server", out var s) ? s.GetString() ?? "iperf.he.net" : "iperf.he.net";
        var duration = options.TryGetProperty("duration", out var d) ? d.GetInt32() : 60;
        var reverse = !options.TryGetProperty("reverse", out var r) || r.GetBoolean();
        var args = new List<string> { "-c", server, "-t", duration.ToString() };
        if (reverse) args.Add("-R");
        return await Required("iperf3", args, log, ct);
    }

    private static async Task<ServiceResult> BleachBit(Action<string> log, CancellationToken ct)
    {
        var path = AppServices.RequiredPrograms.GetExePath("bleachbit") ?? throw new InvalidOperationException("BleachBit not found");
        var console = Path.Combine(Path.GetDirectoryName(path)!, "bleachbit_console.exe");
        var exe = File.Exists(console) ? console : path;
        log($"Running {exe}");
        var result = await ProcessRunner.RunAsync(exe, ["--clean", "system.tmp", "system.recycle_bin", "system.logs"], cancellationToken: ct);
        log(result.Combined);
        return FromProcess("BleachBit", result);
    }

    private static async Task<ServiceResult> DriveCleanup(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var test = options.TryGetProperty("test_only", out var t) && t.GetBoolean();
        var args = test ? new[] { "-t" } : Array.Empty<string>();
        return await Required("drivecleanup", args, log, ct);
    }

    private static async Task<ServiceResult> Dism(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var scanOnly = options.TryGetProperty("scan_only", out var s) && s.GetBoolean();
        var args = scanOnly
            ? new[] { "/Online", "/Cleanup-Image", "/ScanHealth" }
            : new[] { "/Online", "/Cleanup-Image", "/RestoreHealth" };
        return await Cmd("dism", args, log, ct);
    }

    private static async Task<ServiceResult> Chkdsk(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var drive = options.TryGetProperty("drive", out var d) ? d.GetString() ?? "C" : "C";
        var repair = options.TryGetProperty("repair", out var r) && r.GetBoolean();
        var args = new List<string> { $"{drive}:" };
        if (repair) args.AddRange(["/f", "/r"]);
        return await Cmd("chkdsk", args, log, ct);
    }

    private static async Task<ServiceResult> Furmark(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var duration = options.TryGetProperty("duration_seconds", out var d) ? d.GetInt32() : 60;
        return await Required("furmark", [$"/max_time={duration}"], log, ct);
    }

    private static async Task<ServiceResult> WindowsUpdate(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var install = !options.TryGetProperty("install_updates", out var i) || i.GetBoolean();
        log("Scanning Windows Update...");
        var script = install
            ? "Install-Module PSWindowsUpdate -Force -Scope CurrentUser; Get-WindowsUpdate -AcceptAll -Install -IgnoreReboot"
            : "Install-Module PSWindowsUpdate -Force -Scope CurrentUser; Get-WindowsUpdate";
        var result = await ProcessRunner.RunAsync("powershell", ["-NoProfile", "-Command", script], timeoutMs: 0, onLine: log, cancellationToken: ct);
        return FromProcess("Windows Update", result);
    }

    private static async Task<ServiceResult> Energy(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var duration = options.TryGetProperty("duration", out var d) ? d.GetInt32() : 10;
        var html = DataDirectory.PathTo("reports", "energy-report.html");
        var result = await ProcessRunner.RunAsync("powercfg", ["/energy", "/duration", duration.ToString(), "/output", html], cancellationToken: ct);
        log(result.Combined);
        return FromProcess("Energy report", result, html);
    }

    private static async Task<ServiceResult> Battery(Action<string> log, CancellationToken ct)
    {
        var html = DataDirectory.PathTo("reports", "battery-report.html");
        var result = await ProcessRunner.RunAsync("powercfg", ["/batteryreport", "/output", html], cancellationToken: ct);
        log(result.Combined);
        return FromProcess("Battery report", result, html);
    }

    private static async Task<ServiceResult> DriverAudit(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var showAll = options.TryGetProperty("show_all", out var s) && s.GetBoolean();
        var result = await ProcessRunner.RunAsync("driverquery", ["/v", "/fo", "csv"], cancellationToken: ct);
        log(result.Combined);
        return new ServiceResult
        {
            Success = result.Success,
            Logs = [result.Combined],
            Findings =
            [
                new ServiceFinding
                {
                    Severity = FindingSeverity.Info,
                    Title = showAll ? "All drivers" : "Driver inventory",
                    Description = $"{result.Stdout.Split('\n').Length} driver rows collected."
                }
            ]
        };
    }

    private static ServiceResult InstalledSoftware(JsonElement options, Action<string> log)
    {
        var includeUpdates = options.TryGetProperty("include_updates", out var u) && u.GetBoolean();
        log("Querying installed software from registry...");
        var programs = SoftwareInventory.List(includeUpdates);
        var names = programs.Select(p => $"{p.Name} {p.Version}").ToList();
        log($"Found {names.Count} programs");
        return new ServiceResult
        {
            Success = true,
            Logs = names,
            Findings =
            [
                new ServiceFinding
                {
                    Severity = FindingSeverity.Info,
                    Title = "Installed software",
                    Description = $"{names.Count} packages from Uninstall registry keys"
                }
            ]
        };
    }

    private static async Task<ServiceResult> NetworkConfig(Action<string> log, CancellationToken ct)
    {
        var result = await ProcessRunner.RunAsync("ipconfig", ["/all"], timeoutMs: 30_000, cancellationToken: ct);
        log(result.Combined);
        return FromProcess("Network configuration", result);
    }

    private static async Task<ServiceResult> UsbStability(JsonElement options, Action<string> log, CancellationToken ct)
    {
        var intensity = options.TryGetProperty("test_intensity", out var i) ? i.GetString() ?? "standard" : "standard";
        var verify = !options.TryGetProperty("verify_integrity", out var v) || v.GetBoolean();
        var usb = DriveInfo.GetDrives().FirstOrDefault(d => d.IsReady && d.DriveType == DriveType.Removable);
        if (usb is null)
        {
            return new ServiceResult
            {
                Success = false,
                Error = "No removable USB drive found",
                Findings = [new() { Severity = FindingSeverity.Warning, Title = "No USB", Description = "Insert a USB drive to test." }]
            };
        }

        var size = intensity switch { "light" => 8 * 1024 * 1024, "heavy" => 256 * 1024 * 1024, _ => 32 * 1024 * 1024 };
        var file = Path.Combine(usb.RootDirectory.FullName, "rustservice-usb-test.bin");
        var data = new byte[size];
        Random.Shared.NextBytes(data);
        log($"Writing {size} bytes to {file}");
        await File.WriteAllBytesAsync(file, data, ct);
        if (verify)
        {
            var read = await File.ReadAllBytesAsync(file, ct);
            if (!read.SequenceEqual(data)) throw new InvalidOperationException("USB verification failed");
            log("Integrity check passed");
        }

        File.Delete(file);
        return Ok("usb-stability", "USB test passed", $"Wrote and verified {size} bytes on {usb.Name}");
    }

    private static ServiceResult StartupOptimize(JsonElement options, Action<string> log)
    {
        var disable = options.TryGetProperty("disable_unnecessary", out var d) && d.GetBoolean();
        var items = AppServices.Startup.GetAll();
        log($"Found {items.Count} startup items");
        var findings = items.Select(item => new ServiceFinding
        {
            Severity = item.Enabled ? FindingSeverity.Info : FindingSeverity.Success,
            Title = item.Name,
            Description = $"{item.Source}: {item.Command}"
        }).ToList();
        if (disable)
        {
            log("Automatic disable is left to the technician; listing items only.");
        }

        return new ServiceResult { Success = true, Findings = findings, Logs = items.Select(i => i.Name).ToList() };
    }

    private static async Task<ServiceResult> Required(string programId, IEnumerable<string> args, Action<string> log, CancellationToken ct)
    {
        var path = AppServices.RequiredPrograms.GetExePath(programId) ?? throw new InvalidOperationException($"{programId} not found");
        log($"Running {path}");
        var result = await ProcessRunner.RunAsync(path, args, cancellationToken: ct, onLine: log);
        return FromProcess(programId, result);
    }

    private static async Task<ServiceResult> Cmd(string file, IEnumerable<string> args, Action<string> log, CancellationToken ct)
    {
        log($"Running {file}");
        var result = await ProcessRunner.RunAsync(file, args, cancellationToken: ct, onLine: log);
        return FromProcess(file, result);
    }

    private static async Task<ServiceResult> External(string file, string title, IEnumerable<string> args, Action<string> log, CancellationToken ct)
    {
        log($"Running {file}");
        var result = await ProcessRunner.RunAsync(file, args, cancellationToken: ct, onLine: log);
        return FromProcess(title, result);
    }

    private static ServiceResult FromProcess(string title, ProcessResult result, string? artifact = null) =>
        new()
        {
            Success = result.Success,
            Error = result.Success ? null : result.Stderr,
            Logs = result.Combined.Split('\n').Select(l => l.TrimEnd()).Where(l => l.Length > 0).ToList(),
            Findings =
            [
                new ServiceFinding
                {
                    Severity = result.Success ? FindingSeverity.Success : FindingSeverity.Error,
                    Title = title,
                    Description = artifact ?? (result.Success ? "Completed successfully." : result.Stderr)
                }
            ]
        };

    private static ServiceResult Ok(string id, string title, string description) =>
        new()
        {
            ServiceId = id,
            Success = true,
            Findings = [new ServiceFinding { Severity = FindingSeverity.Success, Title = title, Description = description }]
        };
}
