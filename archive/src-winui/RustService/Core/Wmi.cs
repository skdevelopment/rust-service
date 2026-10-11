using System.Management;

namespace RustService.Core;

public static class Wmi
{
    public static IEnumerable<ManagementObject> Query(string query, string scope = @"\\.\root\cimv2")
    {
        using var searcher = new ManagementObjectSearcher(scope, query);
        foreach (ManagementObject obj in searcher.Get())
        {
            yield return obj;
        }
    }

    public static string? Str(ManagementBaseObject obj, string name) =>
        obj[name]?.ToString();

    public static ulong U64(ManagementBaseObject obj, string name) =>
        Convert.ToUInt64(obj[name] ?? 0UL);

    public static uint U32(ManagementBaseObject obj, string name) =>
        Convert.ToUInt32(obj[name] ?? 0U);
}
