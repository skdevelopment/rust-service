namespace RustService.Models;

public sealed class ProgramItem
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public string Version { get; set; } = "";
    public string ExePath { get; set; } = "";
    public string? IconPath { get; set; }
    public bool IsCli { get; set; }
    public uint LaunchCount { get; set; }
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public DateTime? LastLaunched { get; set; }
}

public sealed class ProgramConfig
{
    public List<ProgramItem> Programs { get; set; } = [];
}

public sealed class ScriptItem
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public string ScriptType { get; set; } = "powershell";
    public string Content { get; set; } = "";
    public bool RunAsAdmin { get; set; }
    public uint RunCount { get; set; }
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public DateTime? LastRun { get; set; }
}

public sealed class ScriptConfig
{
    public List<ScriptItem> Scripts { get; set; } = [];
}

public sealed class RequiredProgramDef
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public List<string> ExeNames { get; set; } = [];
    public string? Url { get; set; }
}

public sealed class RequiredProgramStatus
{
    public RequiredProgramDef Definition { get; set; } = new();
    public bool Found { get; set; }
    public string? Path { get; set; }
    public bool IsCustom { get; set; }
}
