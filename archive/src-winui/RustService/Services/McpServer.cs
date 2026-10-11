using System.Net;
using System.Text;
using System.Text.Json;
using RustService.Core;

namespace RustService.Services;

public sealed class McpServer : IDisposable
{
    private HttpListener? listener;
    private CancellationTokenSource? cts;
    private string apiKey = "";

    public bool IsRunning => listener?.IsListening == true;
    public int BoundPort { get; private set; }

    public void Start(int? port = null, string? key = null, bool ignoreEnabled = false)
    {
        var settings = AppServices.Settings.Current.Agent;
        if (!ignoreEnabled && !settings.McpServerEnabled) return;
        Stop();
        apiKey = key ?? settings.McpApiKey ?? "";
        if (string.IsNullOrWhiteSpace(apiKey))
        {
            apiKey = Guid.NewGuid().ToString("N");
            settings.McpApiKey = apiKey;
            AppServices.Settings.Save();
        }

        BoundPort = port ?? settings.McpPort;
        if (BoundPort <= 0) BoundPort = 8377;
        listener = new HttpListener();
        listener.Prefixes.Add($"http://127.0.0.1:{BoundPort}/");
        listener.Start();
        cts = new CancellationTokenSource();
        _ = Task.Run(() => Loop(cts.Token));
    }

    public void Stop()
    {
        cts?.Cancel();
        try { listener?.Stop(); } catch { /* ignore */ }
        try { listener?.Close(); } catch { /* ignore */ }
        listener = null;
        BoundPort = 0;
    }

    private async Task Loop(CancellationToken token)
    {
        while (!token.IsCancellationRequested && listener is not null)
        {
            HttpListenerContext ctx;
            try { ctx = await listener.GetContextAsync(); }
            catch { break; }

            _ = Task.Run(() => Handle(ctx), token);
        }
    }

    private async Task Handle(HttpListenerContext ctx)
    {
        try
        {
            var path = ctx.Request.Url?.AbsolutePath ?? "/";
            if (ctx.Request.HttpMethod == "GET" && path is "/" or "/health")
            {
                await Write(ctx, 200, """{"status":"ok"}""");
                return;
            }

            if (path != "/mcp" || ctx.Request.HttpMethod != "POST")
            {
                await Write(ctx, 404, """{"error":"not found"}""");
                return;
            }

            var auth = ctx.Request.Headers["Authorization"];
            if (string.IsNullOrWhiteSpace(apiKey) || auth != $"Bearer {apiKey}")
            {
                await Write(ctx, 401, """{"error":"unauthorized"}""");
                return;
            }

            using var reader = new StreamReader(ctx.Request.InputStream);
            var body = await reader.ReadToEndAsync();
            using var doc = JsonDocument.Parse(string.IsNullOrWhiteSpace(body) ? "{}" : body);
            var method = doc.RootElement.TryGetProperty("method", out var m) ? m.GetString() : "";
            JsonElement id = default;
            var hasId = doc.RootElement.TryGetProperty("id", out id);
            if (method == "notifications/initialized")
            {
                ctx.Response.StatusCode = 204;
                ctx.Response.Close();
                return;
            }

            object payload;
            if (method is "initialize" or "tools/list" or "tools/call")
            {
                object result = method switch
                {
                    "initialize" => new
                    {
                        protocolVersion = "2024-11-05",
                        capabilities = new { tools = new { listChanged = false } },
                        serverInfo = new { name = "RustService", version = "0.7.0" }
                    },
                    "tools/list" => new { tools = Tools() },
                    _ => await Call(doc.RootElement)
                };
                payload = new Dictionary<string, object?>
                {
                    ["jsonrpc"] = "2.0",
                    ["id"] = hasId ? ToPlain(id) : 1,
                    ["result"] = result
                };
            }
            else
            {
                payload = new Dictionary<string, object?>
                {
                    ["jsonrpc"] = "2.0",
                    ["id"] = hasId ? ToPlain(id) : null,
                    ["error"] = new { code = -32601, message = "Method not found" }
                };
            }

            await Write(ctx, 200, JsonSerializer.Serialize(payload, JsonUtil.Options));
        }
        catch (Exception ex)
        {
            try { await Write(ctx, 500, JsonSerializer.Serialize(new { error = ex.Message }, JsonUtil.Options)); }
            catch { /* ignore */ }
        }
    }

    private static object ToPlain(JsonElement id) => id.ValueKind switch
    {
        JsonValueKind.Number when id.TryGetInt64(out var n) => n,
        JsonValueKind.String => id.GetString() ?? "",
        _ => id.ToString()
    };

    public static object[] Tools() =>
    [
        Tool("execute_command", "Run a PowerShell command", "command", "reason"),
        Tool("read_file", "Read a file", "path"),
        Tool("write_file", "Write a file", "path", "content"),
        Tool("list_dir", "List a directory", "path"),
        Tool("move_file", "Move a file", "src", "dest"),
        Tool("copy_file", "Copy a file", "src", "dest"),
        Tool("get_system_info", "Get system info"),
        Tool("search_web", "Search the web", "query"),
        Tool("list_programs", "List portable programs"),
        Tool("list_instruments", "List instruments"),
        Tool("run_instrument", "Run a custom script", "name")
    ];

    private static object Tool(string name, string description, params string[] props) =>
        new
        {
            name,
            description,
            inputSchema = new
            {
                type = "object",
                properties = props.ToDictionary(p => p, _ => new { type = "string" })
            }
        };

    private static async Task<object> Call(JsonElement root)
    {
        var args = root.TryGetProperty("params", out var p) ? p : root;
        var name = args.TryGetProperty("name", out var n) ? n.GetString() : "";
        var arguments = args.TryGetProperty("arguments", out var a) ? a : default;
        string Str(string key)
        {
            if (arguments.ValueKind is JsonValueKind.Undefined or JsonValueKind.Null) return "";
            return arguments.TryGetProperty(key, out var v) ? v.GetString() ?? "" : "";
        }

        try
        {
            return name switch
            {
                "execute_command" => Wrap(await Commands.ExecuteCommand(Str("command"), Str("reason"))),
                "read_file" => Wrap(Commands.AgentReadFile(Str("path"))),
                "write_file" => Wrap(Commands.AgentWriteFile(Str("path"), Str("content"))),
                "list_dir" => Wrap(Commands.AgentListDir(string.IsNullOrWhiteSpace(Str("path")) ? DataDirectory.Root : Str("path"))),
                "move_file" => MoveThenWrap(Str("src"), Str("dest")),
                "copy_file" => CopyThenWrap(Str("src"), Str("dest")),
                "get_system_info" => Wrap(Commands.GetSystemInfo()),
                "search_web" => Wrap(await Commands.SearchWeb(Str("query"))),
                "list_programs" => Wrap(Commands.GetPrograms()),
                "list_instruments" => Wrap(Commands.ListInstruments()),
                "run_instrument" => Wrap(await RunInstrument(Str("name"))),
                _ => new { error = $"Unknown tool {name}" }
            };
        }
        catch (Exception ex)
        {
            return new { error = ex.Message };
        }
    }

    private static object Wrap(object value) => value;

    private static object WriteThenWrap(string path, string content)
    {
        AppServices.Files.Write(path, content);
        return new { ok = true, path };
    }

    private static object MoveThenWrap(string src, string dest)
    {
        Commands.AgentMoveFile(src, dest);
        return new { ok = true, src, dest };
    }

    private static object CopyThenWrap(string src, string dest)
    {
        Commands.AgentCopyFile(src, dest);
        return new { ok = true, src, dest };
    }

    private static async Task<object> RunInstrument(string name)
    {
        var scripts = AppServices.Scripts.GetAll();
        var script = scripts.FirstOrDefault(s => s.Name.Equals(name, StringComparison.OrdinalIgnoreCase))
            ?? throw new InvalidOperationException("Instrument not found");
        return await AppServices.Scripts.RunAsync(script.Id);
    }

    private static async Task Write(HttpListenerContext ctx, int status, string json)
    {
        ctx.Response.StatusCode = status;
        ctx.Response.ContentType = "application/json";
        var bytes = Encoding.UTF8.GetBytes(json);
        await ctx.Response.OutputStream.WriteAsync(bytes);
        ctx.Response.Close();
    }

    public void Dispose() => Stop();
}
