using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class AgentHost
{
    private readonly List<PendingCommand> pending = [];
    private readonly HttpClient http = new();

    public IReadOnlyList<PendingCommand> Pending => pending;

    public PendingCommand Queue(string command, string reason)
    {
        var item = new PendingCommand { Command = command, Reason = reason };
        pending.Add(item);
        AppServices.Memory.InsertCommand(item);
        return item;
    }

    public void ClearPending() => pending.Clear();

    public async Task<PendingCommand> ExecuteDirectAsync(string command, string reason)
    {
        var result = await ProcessRunner.RunAsync("powershell", ["-NoProfile", "-Command", command]);
        var item = new PendingCommand
        {
            Command = command,
            Reason = reason,
            Status = result.Success ? CommandStatus.Executed : CommandStatus.Failed,
            Output = result.Stdout,
            Error = result.Success ? null : result.Stderr
        };
        AppServices.Memory.InsertCommand(item);
        return item;
    }

    public async Task<PendingCommand> ApproveAsync(string id)
    {
        var item = pending.FirstOrDefault(p => p.Id == id) ?? throw new InvalidOperationException("Command not found");
        item.Status = CommandStatus.Approved;
        var result = await ProcessRunner.RunAsync("powershell", ["-NoProfile", "-Command", item.Command]);
        item.Status = result.Success ? CommandStatus.Executed : CommandStatus.Failed;
        item.Output = result.Stdout;
        item.Error = result.Success ? null : result.Stderr;
        pending.Remove(item);
        AppServices.Memory.InsertCommand(item);
        return item;
    }

    public PendingCommand Reject(string id)
    {
        var item = pending.FirstOrDefault(p => p.Id == id) ?? throw new InvalidOperationException("Command not found");
        item.Status = CommandStatus.Rejected;
        pending.Remove(item);
        AppServices.Memory.InsertCommand(item);
        return item;
    }

    public async Task<ToolExecutionResponse> ExecuteOrQueueAsync(string command, string reason)
    {
        var mode = AppServices.Settings.Current.Agent.ApprovalMode;
        var allow = mode == ApprovalMode.Yolo
            || (mode == ApprovalMode.Whitelist && AppServices.Settings.Current.Agent.WhitelistedCommands.Any(p => Regex.IsMatch(command, p)));
        if (!allow)
        {
            var queued = Queue(command, reason);
            return new ToolExecutionResponse { Success = false, RequiresApproval = true, PendingCommandId = queued.Id };
        }

        var result = await ProcessRunner.RunAsync("powershell", ["-NoProfile", "-Command", command]);
        var item = new PendingCommand
        {
            Command = command,
            Reason = reason,
            Status = result.Success ? CommandStatus.Executed : CommandStatus.Failed,
            Output = result.Stdout,
            Error = result.Success ? null : result.Stderr
        };
        AppServices.Memory.InsertCommand(item);
        return new ToolExecutionResponse { Success = result.Success, Result = result.Combined, Error = item.Error };
    }

    public Task<IReadOnlyList<SearchResult>> SearchTavilyAsync(string query, string apiKey)
    {
        AppServices.Settings.Current.Agent.SearchProvider = SearchProvider.Tavily;
        AppServices.Settings.Current.Agent.TavilyApiKey = apiKey;
        return SearchAsync(query);
    }

    public Task<IReadOnlyList<SearchResult>> SearchSearxngAsync(string query, string instanceUrl)
    {
        AppServices.Settings.Current.Agent.SearchProvider = SearchProvider.Searxng;
        AppServices.Settings.Current.Agent.SearxngUrl = instanceUrl;
        return SearchAsync(query);
    }

    public async Task<IReadOnlyList<SearchResult>> SearchAsync(string query)
    {
        var settings = AppServices.Settings.Current.Agent;
        if (settings.SearchProvider == SearchProvider.Tavily && !string.IsNullOrWhiteSpace(settings.TavilyApiKey))
        {
            var payload = JsonSerializer.Serialize(new { api_key = settings.TavilyApiKey, query, max_results = 5 });
            using var resp = await http.PostAsync("https://api.tavily.com/search", new StringContent(payload, Encoding.UTF8, "application/json"));
            var json = await resp.Content.ReadAsStringAsync();
            using var doc = JsonDocument.Parse(json);
            return doc.RootElement.GetProperty("results").EnumerateArray()
                .Select(r => new SearchResult
                {
                    Title = r.GetProperty("title").GetString() ?? "",
                    Url = r.GetProperty("url").GetString() ?? "",
                    Snippet = r.TryGetProperty("content", out var c) ? c.GetString() ?? "" : "",
                    Score = r.TryGetProperty("score", out var s) ? s.GetDouble() : null
                }).ToList();
        }

        if (settings.SearchProvider == SearchProvider.Searxng && !string.IsNullOrWhiteSpace(settings.SearxngUrl))
        {
            var url = $"{settings.SearxngUrl.TrimEnd('/')}/search?q={Uri.EscapeDataString(query)}&format=json";
            var json = await http.GetStringAsync(url);
            using var doc = JsonDocument.Parse(json);
            return doc.RootElement.GetProperty("results").EnumerateArray()
                .Select(r => new SearchResult
                {
                    Title = r.GetProperty("title").GetString() ?? "",
                    Url = r.GetProperty("url").GetString() ?? "",
                    Snippet = r.TryGetProperty("content", out var c) ? c.GetString() ?? "" : ""
                }).ToList();
        }

        return [];
    }

    public async Task<string> ChatAsync(string conversationId, string userMessage, CancellationToken cancellationToken = default)
    {
        var settings = AppServices.Settings.Current.Agent;
        var convo = AppServices.Memory.GetConversation(conversationId) ?? throw new InvalidOperationException("Conversation not found");
        convo.Messages.Add(new ConversationMessage
        {
            Id = Guid.NewGuid().ToString(),
            ConversationId = conversationId,
            Role = "user",
            Content = userMessage,
            CreatedAt = DateTime.UtcNow.ToString("O")
        });

        var apiKey = KeyFor(settings);
        if (string.IsNullOrWhiteSpace(apiKey) && settings.Provider is not AgentProvider.Ollama)
        {
            var notice = "Set an API key in Settings → AI Agent before chatting.";
            convo.Messages.Add(Assistant(conversationId, notice));
            AppServices.Memory.SaveMessages(conversationId, convo.Messages);
            return notice;
        }

        var system = string.IsNullOrWhiteSpace(settings.SystemPrompt)
            ? "You are RustService, a Windows repair technician assistant. Prefer concise actionable steps. Use tools when you need live system data or to run commands."
            : settings.SystemPrompt;

        var messages = new List<Dictionary<string, object?>>
        {
            new() { ["role"] = "system", ["content"] = system }
        };
        foreach (var m in convo.Messages)
        {
            messages.Add(new() { ["role"] = m.Role, ["content"] = m.Content });
        }

        string content;
        try
        {
            content = settings.Provider == AgentProvider.Anthropic
                ? await AnthropicAsync(settings, apiKey, system, convo.Messages, cancellationToken)
                : await OpenAiLoopAsync(settings, apiKey, messages, cancellationToken);
        }
        catch (Exception ex)
        {
            content = $"Provider error: {ex.Message}";
        }

        convo.Messages.Add(Assistant(conversationId, content));
        AppServices.Memory.SaveMessages(conversationId, convo.Messages);
        return content;
    }

    private async Task<string> OpenAiLoopAsync(AgentSettings settings, string? apiKey, List<Dictionary<string, object?>> messages, CancellationToken ct)
    {
        var tools = ToolDefs();
        string? final = null;
        for (var i = 0; i < 8; i++)
        {
            var payload = new Dictionary<string, object?>
            {
                ["model"] = settings.Model,
                ["messages"] = messages,
                ["tools"] = tools
            };
            using var req = new HttpRequestMessage(HttpMethod.Post, Endpoint(settings));
            if (!string.IsNullOrWhiteSpace(apiKey)) req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", apiKey);
            req.Content = new StringContent(JsonSerializer.Serialize(payload), Encoding.UTF8, "application/json");
            using var resp = await http.SendAsync(req, ct);
            var body = await resp.Content.ReadAsStringAsync(ct);
            using var doc = JsonDocument.Parse(body);
            if (!doc.RootElement.TryGetProperty("choices", out var choices) || choices.GetArrayLength() == 0)
            {
                return body;
            }

            var message = choices[0].GetProperty("message");
            var toolCalls = message.TryGetProperty("tool_calls", out var tc) && tc.ValueKind == JsonValueKind.Array ? tc : default;
            var text = message.TryGetProperty("content", out var c) ? c.GetString() : null;
            if (toolCalls.ValueKind != JsonValueKind.Array || toolCalls.GetArrayLength() == 0)
            {
                return text ?? body;
            }

            messages.Add(JsonSerializer.Deserialize<Dictionary<string, object?>>(message.GetRawText())!);
            foreach (var call in toolCalls.EnumerateArray())
            {
                var id = call.GetProperty("id").GetString() ?? Guid.NewGuid().ToString();
                var fn = call.GetProperty("function");
                var name = fn.GetProperty("name").GetString() ?? "";
                var args = fn.TryGetProperty("arguments", out var a) ? a.GetString() ?? "{}" : "{}";
                var result = await InvokeTool(name, args);
                messages.Add(new Dictionary<string, object?>
                {
                    ["role"] = "tool",
                    ["tool_call_id"] = id,
                    ["content"] = result
                });
            }

            final = text;
        }

        return final ?? "Tool loop limit reached.";
    }

    private async Task<string> AnthropicAsync(AgentSettings settings, string? apiKey, string system, List<ConversationMessage> history, CancellationToken ct)
    {
        var payload = JsonSerializer.Serialize(new
        {
            model = settings.Model,
            max_tokens = 2048,
            system,
            messages = history.Where(m => m.Role is "user" or "assistant").Select(m => new { role = m.Role, content = m.Content })
        });
        using var req = new HttpRequestMessage(HttpMethod.Post, "https://api.anthropic.com/v1/messages");
        req.Headers.Add("x-api-key", apiKey ?? "");
        req.Headers.Add("anthropic-version", "2023-06-01");
        req.Content = new StringContent(payload, Encoding.UTF8, "application/json");
        using var resp = await http.SendAsync(req, ct);
        var body = await resp.Content.ReadAsStringAsync(ct);
        using var doc = JsonDocument.Parse(body);
        if (doc.RootElement.TryGetProperty("content", out var content) && content.ValueKind == JsonValueKind.Array)
        {
            return string.Join("\n", content.EnumerateArray()
                .Where(p => p.TryGetProperty("text", out _))
                .Select(p => p.GetProperty("text").GetString()));
        }

        return body;
    }

    private async Task<string> InvokeTool(string name, string argsJson)
    {
        using var doc = JsonDocument.Parse(string.IsNullOrWhiteSpace(argsJson) ? "{}" : argsJson);
        var args = doc.RootElement;
        string Str(string key) => args.TryGetProperty(key, out var v) ? v.GetString() ?? "" : "";
        try
        {
            object result = name switch
            {
                "execute_command" => await ExecuteOrQueueAsync(Str("command"), Str("reason")),
                "get_system_info" => AppServices.SystemInfo.Get(),
                "list_programs" => AppServices.Programs.GetAll(),
                "find_exe" => AppServices.Files.FindExe(string.IsNullOrWhiteSpace(Str("query")) ? Str("name") : Str("query")) ?? "",
                "list_instruments" => AppServices.Scripts.GetAll().Select(s => s.Name).ToList(),
                "run_instrument" => await RunInstrument(Str("name")),
                "search_web" => await SearchAsync(Str("query")),
                "read_file" => AppServices.Files.Read(Str("path")),
                "write_file" => WriteOk(Str("path"), Str("content")),
                "generate_file" => AppServices.Files.Generate(Str("filename"), Str("content"), Str("description")),
                "list_dir" => AppServices.Files.ListDir(Str("path")),
                "move_file" => MoveOk(Str("src"), Str("dest")),
                "copy_file" => CopyOk(Str("src"), Str("dest")),
                "edit_file" => EditOk(Str("path"), First(Str("oldString"), Str("old_text")), First(Str("newString"), Str("new_text"))),
                "grep" => AppServices.Files.Grep(string.IsNullOrWhiteSpace(Str("path")) ? DataDirectory.Root : Str("path"), Str("pattern")),
                "glob" => AppServices.Files.Glob(string.IsNullOrWhiteSpace(Str("path")) ? DataDirectory.Root : Str("path"), Str("pattern")),
                "save_memory" => AppServices.Memory.SaveMemory(string.IsNullOrWhiteSpace(Str("memory_type")) ? "fact" : Str("memory_type"), Str("content")),
                "search_memories" => AppServices.Memory.SearchMemories(Str("query")),
                "list_services" => ServiceCatalog.Definitions,
                "list_service_presets" => ServiceCatalog.AllPresets(),
                "check_service_requirements" => Commands.ValidateServiceRequirements(StringList(args, "service_ids")),
                "get_service_status" => AppServices.Runner.State,
                "get_service_report" => AppServices.Reports.Get(Str("report_id")),
                "get_report_statistics" => Commands.GetReportStatistics(Str("report_id")),
                "edit_finding" => EditFinding(args),
                "add_finding" => AddFinding(args),
                "remove_finding" => RemoveFinding(args),
                "set_report_summary" => SetOk(() => Commands.SetReportSummary(Str("report_id"), Str("summary"))),
                "set_service_analysis" => SetOk(() => Commands.SetServiceAnalysis(Str("report_id"), Str("service_id"), Str("analysis"))),
                "set_health_score" => SetOk(() => Commands.SetReportHealthScore(Str("report_id"), Int(args, "score"))),
                "generate_report_pdf" => Commands.GenerateReportPdf(Str("report_id"), string.IsNullOrWhiteSpace(Str("output_path")) ? null : Str("output_path")),
                "run_service_queue" => await RunQueue(args),
                "pause_service" => SetOk(Commands.PauseServiceRun),
                "resume_service" => SetOk(Commands.ResumeServiceRun),
                "cancel_service" => SetOk(Commands.CancelServiceRun),
                _ => new { error = $"Unknown tool {name}" }
            };
            return JsonSerializer.Serialize(result, JsonUtil.Options);
        }
        catch (Exception ex)
        {
            return JsonSerializer.Serialize(new { error = ex.Message });
        }
    }

    private static object WriteOk(string path, string content)
    {
        AppServices.Files.Write(path, content);
        return new { status = "success", path };
    }

    private static object MoveOk(string src, string dest)
    {
        AppServices.Files.Move(src, dest);
        return new { status = "success", dest };
    }

    private static object CopyOk(string src, string dest)
    {
        AppServices.Files.Copy(src, dest);
        return new { status = "success", dest };
    }

    private static object EditOk(string path, string oldText, string newText)
    {
        AppServices.Files.Edit(path, oldText, newText);
        return new { status = "success", path };
    }

    private static object SetOk(Action action)
    {
        action();
        return new { status = "success" };
    }

    private static string First(string a, string b) => string.IsNullOrWhiteSpace(a) ? b : a;

    private static int Int(JsonElement args, string key) =>
        args.TryGetProperty(key, out var v) && v.TryGetInt32(out var n) ? n : 0;

    private static IEnumerable<string> StringList(JsonElement args, string key)
    {
        if (!args.TryGetProperty(key, out var v) || v.ValueKind != JsonValueKind.Array) return [];
        return v.EnumerateArray().Select(i => i.GetString() ?? "").Where(s => s.Length > 0);
    }

    private static object EditFinding(JsonElement args)
    {
        Commands.EditReportFinding(
            args.GetProperty("report_id").GetString() ?? "",
            args.GetProperty("service_id").GetString() ?? "",
            Int(args, "finding_index"),
            args.TryGetProperty("severity", out var s) ? s.GetString() : null,
            args.TryGetProperty("title", out var t) ? t.GetString() : null,
            args.TryGetProperty("description", out var d) ? d.GetString() : null,
            args.TryGetProperty("recommendation", out var r) ? r.GetString() : null);
        return new { status = "success" };
    }

    private static object AddFinding(JsonElement args)
    {
        Commands.AddReportFinding(
            args.GetProperty("report_id").GetString() ?? "",
            args.GetProperty("service_id").GetString() ?? "",
            args.TryGetProperty("severity", out var s) ? s.GetString() ?? "info" : "info",
            args.GetProperty("title").GetString() ?? "",
            args.GetProperty("description").GetString() ?? "",
            args.TryGetProperty("recommendation", out var r) ? r.GetString() : null);
        return new { status = "success" };
    }

    private static object RemoveFinding(JsonElement args)
    {
        Commands.RemoveReportFinding(
            args.GetProperty("report_id").GetString() ?? "",
            args.GetProperty("service_id").GetString() ?? "",
            Int(args, "finding_index"));
        return new { status = "success" };
    }

    private static async Task<object> RunQueue(JsonElement args)
    {
        var queue = new List<ServiceQueueItem>();
        if (args.TryGetProperty("queue", out var items) && items.ValueKind == JsonValueKind.Array)
        {
            var order = 0;
            foreach (var item in items.EnumerateArray())
            {
                queue.Add(new ServiceQueueItem
                {
                    ServiceId = item.TryGetProperty("service_id", out var id) ? id.GetString() ?? "" : "",
                    Enabled = !item.TryGetProperty("enabled", out var en) || en.ValueKind != JsonValueKind.False,
                    Order = item.TryGetProperty("order", out var o) && o.TryGetInt32(out var n) ? n : order,
                    Options = item.TryGetProperty("options", out var opt) ? opt.Clone() : JsonDocument.Parse("{}").RootElement.Clone()
                });
                order++;
            }
        }

        var tech = args.TryGetProperty("technician_name", out var tn) ? tn.GetString() : null;
        var customer = args.TryGetProperty("customer_name", out var cn) ? cn.GetString() : null;
        var report = await Commands.RunServices(queue, parallel: false, tech, customer);
        return new { status = "started", report_id = report.Id, message = report.Status.ToString() };
    }

    private static async Task<object> RunInstrument(string name)
    {
        var script = AppServices.Scripts.GetAll().FirstOrDefault(s => s.Name.Equals(name, StringComparison.OrdinalIgnoreCase))
            ?? throw new InvalidOperationException("Instrument not found");
        return await AppServices.Scripts.RunAsync(script.Id);
    }

    private static MemoryType ParseMemory(string value) =>
        Enum.TryParse<MemoryType>(value, true, out var t) ? t : MemoryType.Fact;

    private static object[] ToolDefs() =>
    [
        Fn("execute_command", "Run a PowerShell command (may require approval)", ("command", "string"), ("reason", "string")),
        Fn("get_system_info", "Get hardware and OS information"),
        Fn("list_programs", "List portable programs"),
        Fn("find_exe", "Find an executable by name", ("query", "string")),
        Fn("list_instruments", "List saved scripts"),
        Fn("run_instrument", "Run a saved script by name", ("name", "string")),
        Fn("search_web", "Search the web", ("query", "string")),
        Fn("read_file", "Read a file", ("path", "string")),
        Fn("write_file", "Write a file", ("path", "string"), ("content", "string")),
        Fn("generate_file", "Write a file into the agent workspace", ("filename", "string"), ("content", "string"), ("description", "string")),
        Fn("list_dir", "List a directory", ("path", "string")),
        Fn("move_file", "Move or rename a file", ("src", "string"), ("dest", "string")),
        Fn("copy_file", "Copy a file", ("src", "string"), ("dest", "string")),
        Fn("edit_file", "Replace text in a file", ("path", "string"), ("oldString", "string"), ("newString", "string")),
        Fn("grep", "Search file contents", ("pattern", "string"), ("path", "string")),
        Fn("glob", "Find files by glob", ("pattern", "string"), ("path", "string")),
        Fn("save_memory", "Save a memory", ("memory_type", "string"), ("content", "string")),
        Fn("search_memories", "Search saved memories", ("query", "string")),
        Fn("list_services", "List service definitions"),
        Fn("list_service_presets", "List service presets"),
        Fn("check_service_requirements", "Check required programs for services"),
        Fn("get_service_status", "Get current service run state"),
        Fn("get_service_report", "Get a saved service report", ("report_id", "string")),
        Fn("get_report_statistics", "Get report statistics", ("report_id", "string")),
        Fn("edit_finding", "Edit a report finding", ("report_id", "string"), ("service_id", "string"), ("finding_index", "number")),
        Fn("add_finding", "Add a report finding", ("report_id", "string"), ("service_id", "string"), ("severity", "string"), ("title", "string"), ("description", "string")),
        Fn("remove_finding", "Remove a report finding", ("report_id", "string"), ("service_id", "string"), ("finding_index", "number")),
        Fn("set_report_summary", "Set report executive summary", ("report_id", "string"), ("summary", "string")),
        Fn("set_service_analysis", "Set service analysis text", ("report_id", "string"), ("service_id", "string"), ("analysis", "string")),
        Fn("set_health_score", "Set report health score 0-100", ("report_id", "string"), ("score", "number")),
        Fn("generate_report_pdf", "Generate a report PDF", ("report_id", "string")),
        Fn("run_service_queue", "Run a service queue (technician approval may be required)"),
        Fn("pause_service", "Pause the running service queue"),
        Fn("resume_service", "Resume a paused service queue"),
        Fn("cancel_service", "Cancel the running service queue")
    ];

    private static object Fn(string name, string description, params (string Name, string Type)[] props) =>
        new
        {
            type = "function",
            function = new
            {
                name,
                description,
                parameters = new
                {
                    type = "object",
                    properties = props.ToDictionary(p => p.Name, p => new { type = p.Type }),
                    required = props.Select(p => p.Name).ToArray()
                }
            }
        };

    private static ConversationMessage Assistant(string conversationId, string content) => new()
    {
        Id = Guid.NewGuid().ToString(),
        ConversationId = conversationId,
        Role = "assistant",
        Content = content,
        CreatedAt = DateTime.UtcNow.ToString("O")
    };

    public static string? KeyFor(AgentSettings s) => s.ApiKeys.Get(s.Provider);

    private static string Endpoint(AgentSettings s) => s.Provider switch
    {
        AgentProvider.Xai => "https://api.x.ai/v1/chat/completions",
        AgentProvider.Groq => "https://api.groq.com/openai/v1/chat/completions",
        AgentProvider.OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
        AgentProvider.DeepSeek => "https://api.deepseek.com/chat/completions",
        AgentProvider.Mistral => "https://api.mistral.ai/v1/chat/completions",
        AgentProvider.Google => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        AgentProvider.Ollama => (s.BaseUrl ?? "http://127.0.0.1:11434") + "/v1/chat/completions",
        AgentProvider.Custom => (s.BaseUrl ?? "https://api.openai.com/v1") + "/chat/completions",
        _ => "https://api.openai.com/v1/chat/completions"
    };
}
