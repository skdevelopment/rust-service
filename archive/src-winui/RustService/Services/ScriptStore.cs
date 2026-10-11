using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class ScriptStore
{
    private ScriptConfig config = new();

    public IReadOnlyList<ScriptItem> GetAll()
    {
        Reload();
        return config.Scripts;
    }

    public ScriptItem Add(ScriptItem item)
    {
        Reload();
        config.Scripts.Add(item);
        Persist();
        return item;
    }

    public ScriptItem? Update(ScriptItem item)
    {
        Reload();
        var i = config.Scripts.FindIndex(s => s.Id == item.Id);
        if (i < 0) return null;
        config.Scripts[i] = item;
        Persist();
        return item;
    }

    public bool Delete(string id)
    {
        Reload();
        var removed = config.Scripts.RemoveAll(s => s.Id == id) > 0;
        if (removed) Persist();
        return removed;
    }

    public async Task<ProcessResult> RunAsync(string id, CancellationToken cancellationToken = default)
    {
        Reload();
        var script = config.Scripts.FirstOrDefault(s => s.Id == id)
            ?? throw new InvalidOperationException("Script not found");

        var ext = script.ScriptType.Equals("cmd", StringComparison.OrdinalIgnoreCase) ? ".cmd" : ".ps1";
        var temp = Path.Combine(Path.GetTempPath(), $"rs-{script.Id}{ext}");
        await File.WriteAllTextAsync(temp, script.Content, cancellationToken);

        ProcessResult result;
        if (script.ScriptType.Equals("cmd", StringComparison.OrdinalIgnoreCase))
        {
            if (script.RunAsAdmin)
            {
                ProcessRunner.StartDetached("cmd", ["/c", temp], true);
                result = new ProcessResult(0, "Launched elevated.", "");
            }
            else
            {
                result = await ProcessRunner.RunAsync("cmd", ["/c", temp], cancellationToken: cancellationToken);
            }
        }
        else if (script.RunAsAdmin)
        {
            ProcessRunner.StartDetached("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", temp], true);
            result = new ProcessResult(0, "Launched elevated.", "");
        }
        else
        {
            result = await ProcessRunner.RunAsync("powershell", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", temp], cancellationToken: cancellationToken);
        }

        script.RunCount++;
        script.LastRun = DateTime.UtcNow;
        Persist();
        return result;
    }

    private void Reload() => config = JsonUtil.Load(DataDirectory.PathTo("scripts.json"), new ScriptConfig());
    private void Persist() => JsonUtil.Save(DataDirectory.PathTo("scripts.json"), config);
}
