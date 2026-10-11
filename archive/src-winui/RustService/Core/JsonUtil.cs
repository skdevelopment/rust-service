using System.Text.Json;
using System.Text.Json.Serialization;

namespace RustService.Core;

public static class JsonUtil
{
    public static readonly JsonSerializerOptions Options = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Converters = { new JsonStringEnumConverter() }
    };

    public static T Load<T>(string path, T fallback) where T : class
    {
        if (!File.Exists(path))
        {
            return fallback;
        }

        var json = File.ReadAllText(path);
        return JsonSerializer.Deserialize<T>(json, Options) ?? fallback;
    }

    public static void Save<T>(string path, T value)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, JsonSerializer.Serialize(value, Options));
    }
}
