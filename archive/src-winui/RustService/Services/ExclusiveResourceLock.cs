namespace RustService.Services;

/// <summary>
/// Serializes work that shares exclusive resource tags (network-bandwidth, cpu-stress, etc.).
/// Used by the parallel service runner.
/// </summary>
public sealed class ExclusiveResourceLock
{
    private readonly HashSet<string> active = new(StringComparer.OrdinalIgnoreCase);
    private readonly object sync = new();

    public bool CanAcquire(IEnumerable<string> resources)
    {
        lock (sync)
        {
            return !Overlaps(resources);
        }
    }

    public bool TryAcquire(IEnumerable<string> resources)
    {
        lock (sync)
        {
            var list = resources.Where(r => !string.IsNullOrWhiteSpace(r)).ToArray();
            if (Overlaps(list)) return false;
            foreach (var r in list) active.Add(r);
            return true;
        }
    }

    public void Release(IEnumerable<string> resources)
    {
        lock (sync)
        {
            foreach (var r in resources) active.Remove(r);
        }
    }

    public void Clear()
    {
        lock (sync) active.Clear();
    }

    public string[] Snapshot()
    {
        lock (sync) return [.. active];
    }

    private bool Overlaps(IEnumerable<string> resources) =>
        resources.Any(r => !string.IsNullOrWhiteSpace(r) && active.Contains(r));
}
