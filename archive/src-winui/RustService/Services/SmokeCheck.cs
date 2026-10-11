using System.Net.Http;
using System.Text;
using RustService.Core;

namespace RustService.Services;

/// <summary>Headless backend check used by --headless-init. Not a product feature.</summary>
public static class SmokeCheck
{
    public static string Run()
    {
        var lines = new List<string>();
        void Ok(string name, object? detail) => lines.Add($"OK {name} {detail}");
        void Fail(string name, Exception ex) => lines.Add($"FAIL {name} {ex.Message}");

        try
        {
            Ok("data_dir", DataDirectory.Root);
            Ok("agent_db", File.Exists(DataDirectory.PathTo("agent.db")));
            Ok("settings", Commands.GetSettings().Version);
            Ok("tauri_commands", $"{Commands.TauriCommandNames.Length} == {TauriCommandNames.All.Length}");
            Ok("services", Commands.GetServiceDefinitions().Count);
            Ok("presets", Commands.GetServicePresets().Count);
            Ok("required_programs", Commands.GetRequiredProgramsStatus().Count);
            Ok("system", Commands.GetSystemInfo().Os.Name);
            Ok("nics", Commands.GetDetailedNetworkInfo().Count);
            Ok("startup", Commands.GetStartupItems().Count);
            Ok("event_sources", Commands.GetEventLogSources().Count);
            Ok("bsod", Commands.GetBsodStats().TotalCrashes);
            Ok("programs", Commands.GetPrograms().Count);
            Ok("scripts", Commands.GetScripts().Count);
            Ok("shortcuts", ShortcutCatalog.Groups.Count);
            var convo = Commands.CreateConversation("smoke");
            AppServices.Memory.SaveMemory("fact", "smoke-check");
            Ok("agent_memory", convo.Id);
            var pending = Commands.QueueAgentCommand("whoami", "smoke");
            Commands.RejectCommand(pending.Id);
            Ok("approval", "queued-and-rejected");
            var queue = new[]
            {
                new Models.ServiceQueueItem
                {
                    ServiceId = "disk-space",
                    Enabled = true,
                    Order = 0,
                    Options = System.Text.Json.JsonDocument.Parse("{}").RootElement.Clone()
                }
            };
            var report = Commands.RunServices(queue, parallel: false, "smoke", "smoke").GetAwaiter().GetResult();
            var saved = File.Exists(DataDirectory.PathTo("reports", $"{report.Id}.json"));
            Ok("service_run", $"{report.Status} findings={report.Results.SelectMany(r => r.Findings).Count()} saved={saved} id={report.Id}");
            var parallelQueue = new[]
            {
                new Models.ServiceQueueItem { ServiceId = "disk-space", Enabled = true, Order = 0, Options = queue[0].Options },
                new Models.ServiceQueueItem { ServiceId = "startup-optimize", Enabled = true, Order = 1, Options = queue[0].Options }
            };
            var parallel = Commands.RunServices(parallelQueue, parallel: true, "smoke", "smoke").GetAwaiter().GetResult();
            Ok("service_parallel", $"{parallel.Status} parallel={parallel.ParallelMode} results={parallel.Results.Count}");
        }
        catch (Exception ex)
        {
            Fail("core", ex);
        }

        try
        {
            AppServices.Mcp.Start(ignoreEnabled: true);
            using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(3) };
            var health = http.GetStringAsync($"http://127.0.0.1:{AppServices.Mcp.BoundPort}/health").GetAwaiter().GetResult();
            Ok("mcp_health", health);
            var key = AppServices.Settings.Current.Agent.McpApiKey ?? "";
            using var req = new HttpRequestMessage(HttpMethod.Post, $"http://127.0.0.1:{AppServices.Mcp.BoundPort}/mcp");
            req.Headers.TryAddWithoutValidation("Authorization", "Bearer " + key);
            req.Content = new StringContent("""{"jsonrpc":"2.0","method":"tools/list","id":1}""", Encoding.UTF8, "application/json");
            var list = http.SendAsync(req).GetAwaiter().GetResult().Content.ReadAsStringAsync().GetAwaiter().GetResult();
            Ok("mcp_tools", list.Contains("execute_command", StringComparison.Ordinal) && list.Contains("get_system_info", StringComparison.Ordinal));
            AppServices.Mcp.Stop();
        }
        catch (Exception ex)
        {
            Fail("mcp", ex);
        }

        var text = string.Join(Environment.NewLine, lines);
        File.WriteAllText(DataDirectory.PathTo("logs", "smoke.log"), text, Encoding.UTF8);
        return text;
    }
}
