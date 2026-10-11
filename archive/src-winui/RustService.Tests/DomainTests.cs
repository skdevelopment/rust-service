using System.Net;
using System.Net.Http.Headers;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using RustService.Core;
using RustService.Models;
using RustService.Services;
using Xunit;

namespace RustService.Tests;

public sealed class TempDataHost : IDisposable
{
    public string Root { get; }

    public TempDataHost()
    {
        Root = Path.Combine(Path.GetTempPath(), "rs-winui-tests", Guid.NewGuid().ToString("N"));
        AppServices.Initialize(Root);
    }

    public void Dispose()
    {
        AppServices.Shutdown();
        try { Directory.Delete(Root, true); } catch { /* leftover sqlite handles */ }
    }

    public static JsonElement Json(string raw)
    {
        using var doc = JsonDocument.Parse(raw);
        return doc.RootElement.Clone();
    }
}

public class DataDirectoryTests
{
    [Fact]
    public void Ensure_creates_portable_layout()
    {
        using var host = new TempDataHost();
        Assert.True(Directory.Exists(Path.Combine(host.Root, "programs")));
        Assert.True(Directory.Exists(Path.Combine(host.Root, "reports")));
        Assert.True(Directory.Exists(Path.Combine(host.Root, "scripts")));
        Assert.True(Directory.Exists(Path.Combine(host.Root, "logs")));
        Assert.True(File.Exists(Path.Combine(host.Root, "settings.json")));
        Assert.True(File.Exists(Path.Combine(host.Root, "agent.db")));
        Assert.Equal(host.Root, DataDirectory.Root);
        Assert.False(string.IsNullOrWhiteSpace(File.ReadAllText(Path.Combine(host.Root, "settings.json"))));
    }
}

public class SettingsTests
{
    [Fact]
    public void Settings_round_trip_theme_scheme_animations()
    {
        using var host = new TempDataHost();
        AppServices.Settings.UpdateSetting("appearance.theme", TempDataHost.Json("\"dark\""));
        AppServices.Settings.UpdateSetting("appearance.colorScheme", TempDataHost.Json("\"cobalt\""));
        AppServices.Settings.UpdateSetting("appearance.enableAnimations", TempDataHost.Json("false"));
        AppServices.Settings.Load();
        Assert.Equal("dark", AppServices.Settings.Current.Appearance.Theme);
        Assert.Equal("cobalt", AppServices.Settings.Current.Appearance.ColorScheme);
        Assert.False(AppServices.Settings.Current.Appearance.EnableAnimations);
        var disk = JsonDocument.Parse(File.ReadAllText(Path.Combine(host.Root, "settings.json")));
        Assert.Equal("dark", disk.RootElement.GetProperty("appearance").GetProperty("theme").GetString());
        Assert.Equal("cobalt", disk.RootElement.GetProperty("appearance").GetProperty("colorScheme").GetString());
        Assert.False(disk.RootElement.GetProperty("appearance").GetProperty("enableAnimations").GetBoolean());
    }
}

public class RequiredProgramTests
{
    [Fact]
    public void Search_override_and_validate()
    {
        using var host = new TempDataHost();
        var foundPath = Path.Combine(DataDirectory.PathTo("programs"), "BleachBit", "bleachbit.exe");
        Directory.CreateDirectory(Path.GetDirectoryName(foundPath)!);
        File.WriteAllBytes(foundPath, [0x4D, 0x5A]);
        var status = AppServices.RequiredPrograms.GetStatus(["bleachbit"]);
        Assert.True(status[0].Found);
        Assert.Equal(foundPath, status[0].Path, ignoreCase: true);

        var custom = Path.Combine(host.Root, "custom-bleachbit.exe");
        File.WriteAllBytes(custom, [0x4D, 0x5A]);
        AppServices.RequiredPrograms.SetOverride("bleachbit", custom);
        Assert.Equal(custom, AppServices.RequiredPrograms.GetExePath("bleachbit"));
        Assert.True(AppServices.RequiredPrograms.GetStatus(["bleachbit"])[0].IsCustom);

        var missing = AppServices.RequiredPrograms.Validate(["kvrt", "bleachbit"]);
        Assert.Contains("kvrt", missing);
        Assert.DoesNotContain("bleachbit", missing);
    }
}

public class ProgramStoreTests
{
    [Fact]
    public void Add_list_delete_against_temp_dir()
    {
        using var host = new TempDataHost();
        var exe = Path.Combine(host.Root, "tool.exe");
        File.WriteAllBytes(exe, [0x4D, 0x5A]);
        var item = AppServices.Programs.Add("Tool", "portable", "1.0", exe, true);
        Assert.Contains(AppServices.Programs.GetAll(), p => p.Id == item.Id && p.Name == "Tool" && p.ExePath == exe);
        Assert.True(AppServices.Programs.Delete(item.Id));
        Assert.DoesNotContain(AppServices.Programs.GetAll(), p => p.Id == item.Id);
    }
}

public class ServiceCatalogTests
{
    public static readonly string[] ExpectedIds =
    [
        "restore-point", "ping-test", "disk-space", "winsat", "kvrt-scan", "adwcleaner", "stinger",
        "whynotwin11", "smartctl", "speedtest", "iperf", "bleachbit", "drivecleanup", "heavyload",
        "dism", "sfc", "chkdsk", "furmark", "windows-update", "energy-report", "battery-report",
        "driver-audit", "installed-software", "network-config", "usb-stability", "startup-optimize"
    ];

    [Fact]
    public void Catalog_has_current_26_ids()
    {
        Assert.Equal(26, ServiceCatalog.Definitions.Count);
        Assert.Equal(ExpectedIds.OrderBy(x => x), ServiceCatalog.Definitions.Select(d => d.Id).OrderBy(x => x));
    }
}

public class ServiceRunnerTests
{
    [Fact]
    public async Task Sequential_disk_space_yields_result()
    {
        using var host = new TempDataHost();
        var queue = new[]
        {
            new ServiceQueueItem
            {
                ServiceId = "disk-space",
                Enabled = true,
                Order = 0,
                Options = TempDataHost.Json("{}")
            }
        };
        var report = await AppServices.Runner.RunAsync(queue, parallel: false);
        Assert.NotEmpty(report.Results);
        var result = Assert.Single(report.Results);
        Assert.Equal("disk-space", result.ServiceId);
        Assert.True(result.Success);
        Assert.NotEmpty(result.Findings);
        Assert.True(File.Exists(Path.Combine(host.Root, "reports", $"{report.Id}.json")));
    }

    [Fact]
    public async Task Parallel_scheduler_serializes_overlapping_exclusive_resources()
    {
        using var host = new TempDataHost();
        var started = new List<(string Id, DateTime At)>();
        var ended = new List<(string Id, DateTime At)>();
        var runner = new ServiceRunner(async (id, _, log, ct) =>
        {
            started.Add((id, DateTime.UtcNow));
            log(id);
            await Task.Delay(200, ct);
            ended.Add((id, DateTime.UtcNow));
            return new ServiceResult { ServiceId = id, Success = true, DurationMs = 200 };
        });
        var queue = new[]
        {
            new ServiceQueueItem { ServiceId = "dism", Enabled = true, Order = 0, Options = TempDataHost.Json("{}") },
            new ServiceQueueItem { ServiceId = "sfc", Enabled = true, Order = 1, Options = TempDataHost.Json("{}") }
        };
        Assert.Contains("disk-heavy", ServiceCatalog.Get("dism")!.ExclusiveResources);
        Assert.Contains("disk-heavy", ServiceCatalog.Get("sfc")!.ExclusiveResources);
        var report = await runner.RunAsync(queue, parallel: true);
        Assert.Equal(2, report.Results.Count);
        Assert.True(report.ParallelMode);
        var a = runner.ExecutionLog.Single(x => x.ServiceId == "dism");
        var b = runner.ExecutionLog.Single(x => x.ServiceId == "sfc");
        Assert.True(a.End <= b.Start + TimeSpan.FromMilliseconds(20) || b.End <= a.Start + TimeSpan.FromMilliseconds(20),
            $"overlapping windows dism={a.Start:O}-{a.End:O} sfc={b.Start:O}-{b.End:O}");
    }

    [Fact]
    public void Exclusive_resource_lock_rejects_overlap()
    {
        var gate = new ExclusiveResourceLock();
        Assert.True(gate.TryAcquire(["disk-heavy"]));
        Assert.False(gate.TryAcquire(["disk-heavy"]));
        Assert.True(gate.TryAcquire(["network-bandwidth"]));
        gate.Release(["disk-heavy"]);
        Assert.True(gate.TryAcquire(["disk-heavy"]));
    }

    [Fact]
    public void Queue_from_preset_appends_every_catalog_service()
    {
        using var host = new TempDataHost();
        var diagnostics = ServiceCatalog.AllPresets().First(p => p.Id == "diagnostics");
        var queue = ServiceQueueBuilder.FromPreset(diagnostics);
        Assert.Equal(ServiceCatalog.Definitions.Count, queue.Count);
        foreach (var id in new[] { "chkdsk", "furmark", "heavyload", "stinger" })
        {
            var row = Assert.Single(queue, q => q.ServiceId == id);
            Assert.False(row.Enabled);
        }

        Assert.True(queue.First(q => q.ServiceId == "ping-test").Enabled);
        var added = ServiceQueueBuilder.AddService(queue, "chkdsk");
        Assert.True(added.Enabled);
        Assert.Equal("chkdsk", added.ServiceId);
        Assert.Equal(ServiceCatalog.Definitions.Count, queue.Count);
    }

    [Fact]
    public async Task Parallel_cancel_releases_lock_so_second_run_still_executes_both()
    {
        using var host = new TempDataHost();
        var entered = new System.Collections.Concurrent.ConcurrentBag<string>();
        var fastMode = 0;
        var runner = new ServiceRunner(async (id, _, _, ct) =>
        {
            entered.Add(id);
            if (Volatile.Read(ref fastMode) == 1)
            {
                await Task.Delay(40, ct);
                return new ServiceResult { ServiceId = id, Success = true, DurationMs = 40 };
            }

            try
            {
                await Task.Delay(30_000, ct);
            }
            catch (OperationCanceledException)
            {
                return new ServiceResult { ServiceId = id, Success = false, Error = "Cancelled" };
            }

            return new ServiceResult { ServiceId = id, Success = true };
        });
        var queue = new[]
        {
            new ServiceQueueItem { ServiceId = "dism", Enabled = true, Order = 0, Options = TempDataHost.Json("{}") },
            new ServiceQueueItem { ServiceId = "sfc", Enabled = true, Order = 1, Options = TempDataHost.Json("{}") }
        };
        var first = runner.RunAsync(queue, parallel: true);
        var wait = DateTime.UtcNow;
        while (entered.IsEmpty && (DateTime.UtcNow - wait).TotalSeconds < 3)
        {
            await Task.Delay(20);
        }

        Assert.False(entered.IsEmpty);
        runner.Cancel();
        await first;
        Assert.Empty(runner.ResourceLock.Snapshot());

        entered.Clear();
        Volatile.Write(ref fastMode, 1);
        var second = await runner.RunAsync(queue, parallel: true);
        Assert.Equal(2, second.Results.Count);
        Assert.Contains(second.Results, r => r.ServiceId == "dism");
        Assert.Contains(second.Results, r => r.ServiceId == "sfc");
        Assert.True(second.Results.All(r => r.Success));
        Assert.Empty(runner.ResourceLock.Snapshot());
    }
}

public class AgentApprovalTests
{
    [Fact]
    public async Task Always_queues_yolo_executes_whitelist_matches_only()
    {
        using var host = new TempDataHost();
        AppServices.Settings.Current.Agent.ApprovalMode = ApprovalMode.Always;
        var queued = await AppServices.Agent.ExecuteOrQueueAsync("echo always-mode", "test");
        Assert.True(queued.RequiresApproval);
        Assert.False(string.IsNullOrWhiteSpace(queued.PendingCommandId));

        AppServices.Settings.Current.Agent.ApprovalMode = ApprovalMode.Yolo;
        var yolo = await AppServices.Agent.ExecuteOrQueueAsync("echo yolo-mode", "test");
        Assert.False(yolo.RequiresApproval);
        Assert.True(yolo.Success);
        Assert.Contains("yolo-mode", yolo.Result ?? "", StringComparison.OrdinalIgnoreCase);

        AppServices.Settings.Current.Agent.ApprovalMode = ApprovalMode.Whitelist;
        AppServices.Settings.Current.Agent.WhitelistedCommands = ["^ipconfig"];
        var allowed = await AppServices.Agent.ExecuteOrQueueAsync("ipconfig", "test");
        Assert.False(allowed.RequiresApproval);
        var blocked = await AppServices.Agent.ExecuteOrQueueAsync("echo not-whitelisted", "test");
        Assert.True(blocked.RequiresApproval);
    }

    [Fact]
    public void Sqlite_memory_persists_and_reads_back()
    {
        using var host = new TempDataHost();
        AppServices.Memory.SaveMemory("fact", "this bench PC belongs to the shop");
        AppServices.Memory.InsertCommand(new PendingCommand { Command = "whoami", Reason = "id", Status = CommandStatus.Executed, Output = "ok" });
        var root = host.Root;
        AppServices.Shutdown();
        AppServices.Initialize(root);
        Assert.Contains(AppServices.Memory.GetAllMemories(), m => m.Content.Contains("shop"));
        Assert.Contains(AppServices.Memory.SearchMemories("shop", "fact"), m => m.MemoryType == "fact");
        Assert.Contains(AppServices.Memory.History(), h => h.Command == "whoami" && h.Output == "ok");
        Assert.True(File.Exists(Path.Combine(root, "agent.db")));
    }
}

public class McpServerTests
{
    [Fact]
    public async Task Health_auth_initialize_tools_and_call()
    {
        using var host = new TempDataHost();
        var port = FreePort();
        const string key = "test-mcp-key";
        AppServices.Mcp.Start(port, key, ignoreEnabled: true);
        await Task.Delay(200);
        try
        {
            using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(10) };
            var health = await http.GetStringAsync($"http://127.0.0.1:{port}/health");
            Assert.Contains("ok", health, StringComparison.OrdinalIgnoreCase);
            var evidence = Environment.GetEnvironmentVariable("RS_EVIDENCE");
            if (!string.IsNullOrWhiteSpace(evidence))
            {
                Directory.CreateDirectory(evidence);
                File.WriteAllText(Path.Combine(evidence, "mcp-health.json"), health);
            }

            var unauth = await http.PostAsync($"http://127.0.0.1:{port}/mcp",
                new StringContent("""{"jsonrpc":"2.0","method":"initialize","id":1}""", Encoding.UTF8, "application/json"));
            Assert.Equal(HttpStatusCode.Unauthorized, unauth.StatusCode);

            http.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", key);
            var init = await (await http.PostAsync($"http://127.0.0.1:{port}/mcp",
                new StringContent("""{"jsonrpc":"2.0","method":"initialize","id":1}""", Encoding.UTF8, "application/json"))).Content.ReadAsStringAsync();
            using (var initDoc = JsonDocument.Parse(init))
            {
                Assert.Equal("2.0", initDoc.RootElement.GetProperty("jsonrpc").GetString());
                Assert.Equal("RustService", initDoc.RootElement.GetProperty("result").GetProperty("serverInfo").GetProperty("name").GetString());
            }

            var list = await (await http.PostAsync($"http://127.0.0.1:{port}/mcp",
                new StringContent("""{"jsonrpc":"2.0","method":"tools/list","id":2}""", Encoding.UTF8, "application/json"))).Content.ReadAsStringAsync();
            if (!string.IsNullOrWhiteSpace(evidence))
            {
                File.WriteAllText(Path.Combine(evidence, "mcp-tools.json"), list);
            }
            using (var listDoc = JsonDocument.Parse(list))
            {
                var names = listDoc.RootElement.GetProperty("result").GetProperty("tools").EnumerateArray().Select(t => t.GetProperty("name").GetString()).ToList();
                foreach (var tool in new[] { "execute_command", "read_file", "write_file", "list_dir", "move_file", "copy_file", "get_system_info", "search_web", "list_programs", "list_instruments", "run_instrument" })
                {
                    Assert.Contains(tool, names);
                }
            }

            var callBody = JsonSerializer.Serialize(new
            {
                jsonrpc = "2.0",
                method = "tools/call",
                id = 3,
                @params = new { name = "list_dir", arguments = new { path = host.Root } }
            });
            var call = await (await http.PostAsync($"http://127.0.0.1:{port}/mcp",
                new StringContent(callBody, Encoding.UTF8, "application/json"))).Content.ReadAsStringAsync();
            using var callDoc = JsonDocument.Parse(call);
            Assert.Equal("2.0", callDoc.RootElement.GetProperty("jsonrpc").GetString());
            var resultText = callDoc.RootElement.GetProperty("result").GetRawText();
            Assert.Contains("programs", resultText, StringComparison.OrdinalIgnoreCase);
            Assert.Contains("reports", resultText, StringComparison.OrdinalIgnoreCase);
        }
        finally
        {
            AppServices.Mcp.Stop();
        }
    }

    private static int FreePort()
    {
        var listener = new TcpListener(IPAddress.Loopback, 0);
        listener.Start();
        var port = ((IPEndPoint)listener.LocalEndpoint).Port;
        listener.Stop();
        return port;
    }
}
