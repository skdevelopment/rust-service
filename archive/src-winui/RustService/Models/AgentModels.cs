namespace RustService.Models;

public enum AgentProvider { OpenAI, Anthropic, Xai, Google, Mistral, DeepSeek, Groq, OpenRouter, Ollama, Custom }
public enum ApprovalMode { Always, Whitelist, Yolo }
public enum CommandStatus { Pending, Approved, Rejected, Executed, Failed }
public enum SearchProvider { Tavily, Searxng, None }
public enum McpTransportType { Sse, Http }
public enum FileCategory { Text, Code, Document, Image, Media, Binary }
public enum FileSource { Upload, Generated, Filesystem }
public enum MemoryType { Fact, Solution, Conversation, Instruction }

public sealed class ProviderApiKeys
{
    public string? Openai { get; set; }
    public string? Anthropic { get; set; }
    public string? Xai { get; set; }
    public string? Google { get; set; }
    public string? Mistral { get; set; }
    public string? Deepseek { get; set; }
    public string? Groq { get; set; }
    public string? Openrouter { get; set; }
    public string? Custom { get; set; }

    public string? Get(AgentProvider provider) => provider switch
    {
        AgentProvider.OpenAI => Openai,
        AgentProvider.Anthropic => Anthropic,
        AgentProvider.Xai => Xai,
        AgentProvider.Google => Google,
        AgentProvider.Mistral => Mistral,
        AgentProvider.DeepSeek => Deepseek,
        AgentProvider.Groq => Groq,
        AgentProvider.OpenRouter => Openrouter,
        AgentProvider.Ollama => Custom,
        AgentProvider.Custom => Custom,
        _ => Custom
    };

    public void Set(AgentProvider provider, string? key)
    {
        switch (provider)
        {
            case AgentProvider.OpenAI: Openai = key; break;
            case AgentProvider.Anthropic: Anthropic = key; break;
            case AgentProvider.Xai: Xai = key; break;
            case AgentProvider.Google: Google = key; break;
            case AgentProvider.Mistral: Mistral = key; break;
            case AgentProvider.DeepSeek: Deepseek = key; break;
            case AgentProvider.Groq: Groq = key; break;
            case AgentProvider.OpenRouter: Openrouter = key; break;
            default: Custom = key; break;
        }
    }
}

public sealed class AgentSettings
{
    public AgentProvider Provider { get; set; } = AgentProvider.OpenAI;
    public string Model { get; set; } = "gpt-4o-mini";
    public ProviderApiKeys ApiKeys { get; set; } = new();
    public string? BaseUrl { get; set; }
    public ApprovalMode ApprovalMode { get; set; } = ApprovalMode.Always;
    public List<string> WhitelistedCommands { get; set; } =
    [
        "^ipconfig", "^ping ", "^systeminfo$", "^tasklist$", "^hostname$", "^whoami$"
    ];
    public SearchProvider SearchProvider { get; set; } = SearchProvider.None;
    public string? TavilyApiKey { get; set; }
    public string? SearxngUrl { get; set; }
    public string? SystemPrompt { get; set; }
    public bool McpServerEnabled { get; set; }
    public string? McpApiKey { get; set; }
    public int McpPort { get; set; } = 8377;
    public List<McpServerConfig> McpServers { get; set; } = [];
}

public sealed class McpServerConfig
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Url { get; set; } = "";
    public McpTransportType TransportType { get; set; } = McpTransportType.Sse;
    public bool Enabled { get; set; }
    public string? ApiKey { get; set; }
    public Dictionary<string, string>? Headers { get; set; }
}

public sealed class PendingCommand
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public string Command { get; set; } = "";
    public string Reason { get; set; } = "";
    public string CreatedAt { get; set; } = DateTime.UtcNow.ToString("O");
    public CommandStatus Status { get; set; } = CommandStatus.Pending;
    public string? Output { get; set; }
    public string? Error { get; set; }
}

public sealed class SearchResult
{
    public string Title { get; set; } = "";
    public string Url { get; set; } = "";
    public string Snippet { get; set; } = "";
    public double? Score { get; set; }
}

public sealed class Conversation
{
    public string Id { get; set; } = "";
    public string Title { get; set; } = "";
    public string CreatedAt { get; set; } = "";
    public string UpdatedAt { get; set; } = "";
}

public sealed class ConversationMessage
{
    public string Id { get; set; } = "";
    public string ConversationId { get; set; } = "";
    public string Role { get; set; } = "";
    public string Content { get; set; } = "";
    public string CreatedAt { get; set; } = "";
}

public sealed class ConversationWithMessages
{
    public string Id { get; set; } = "";
    public string Title { get; set; } = "";
    public string CreatedAt { get; set; } = "";
    public string UpdatedAt { get; set; } = "";
    public List<ConversationMessage> Messages { get; set; } = [];
}

public sealed class ToolExecutionResponse
{
    public bool Success { get; set; }
    public string? Result { get; set; }
    public string? Error { get; set; }
    public bool RequiresApproval { get; set; }
    public string? PendingCommandId { get; set; }
}

public sealed class FileAttachment
{
    public string Id { get; set; } = "";
    public FileSource Source { get; set; }
    public string OriginalName { get; set; } = "";
    public string StoredName { get; set; } = "";
    public string MimeType { get; set; } = "";
    public FileCategory Category { get; set; }
    public long Size { get; set; }
    public string StoredPath { get; set; } = "";
    public string? ThumbnailPath { get; set; }
    public string? Content { get; set; }
    public string? Encoding { get; set; }
    public int? LineCount { get; set; }
    public string Checksum { get; set; } = "";
    public string UploadedAt { get; set; } = DateTime.UtcNow.ToString("O");
    public string? ExpiresAt { get; set; }
}

public sealed class PathValidationResult
{
    public bool Valid { get; set; }
    public string? SanitizedPath { get; set; }
    public string? Error { get; set; }
    public bool WithinSandbox { get; set; }
}

public sealed class MemoryEntry
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public MemoryType MemoryType { get; set; } = MemoryType.Fact;
    public string Content { get; set; } = "";
    public string? Metadata { get; set; }
    public string CreatedAt { get; set; } = DateTime.UtcNow.ToString("O");
}
