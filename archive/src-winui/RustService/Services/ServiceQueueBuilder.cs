using System.Text.Json;
using RustService.Models;

namespace RustService.Services;

/// <summary>
/// Builds the technician queue the way the Tauri UI does: preset rows first,
/// then every remaining catalog service disabled with its default options.
/// </summary>
public static class ServiceQueueBuilder
{
    public static List<ServiceQueueItem> FromPreset(ServicePreset preset)
    {
        var queue = preset.Services.Select((s, i) => new ServiceQueueItem
        {
            ServiceId = s.ServiceId,
            Enabled = s.Enabled,
            Order = i,
            Options = CloneOptions(s.Options, ServiceCatalog.Get(s.ServiceId))
        }).ToList();
        AppendMissingCatalogServices(queue);
        return queue;
    }

    public static void AppendMissingCatalogServices(List<ServiceQueueItem> queue)
    {
        var present = queue.Select(q => q.ServiceId).ToHashSet(StringComparer.OrdinalIgnoreCase);
        foreach (var def in ServiceCatalog.Definitions)
        {
            if (present.Contains(def.Id)) continue;
            queue.Add(new ServiceQueueItem
            {
                ServiceId = def.Id,
                Enabled = false,
                Order = queue.Count,
                Options = DefaultOptions(def)
            });
        }

        for (var i = 0; i < queue.Count; i++) queue[i].Order = i;
    }

    public static ServiceQueueItem AddService(List<ServiceQueueItem> queue, string serviceId)
    {
        var def = ServiceCatalog.Get(serviceId)
            ?? throw new InvalidOperationException($"Unknown service {serviceId}");
        var existing = queue.FirstOrDefault(q => q.ServiceId.Equals(serviceId, StringComparison.OrdinalIgnoreCase));
        if (existing is not null)
        {
            existing.Enabled = true;
            return existing;
        }

        var item = new ServiceQueueItem
        {
            ServiceId = serviceId,
            Enabled = true,
            Order = queue.Count,
            Options = DefaultOptions(def)
        };
        queue.Add(item);
        return item;
    }

    public static JsonElement DefaultOptions(ServiceDefinition def)
    {
        if (def.Options.Count == 0) return JsonDocument.Parse("{}").RootElement.Clone();
        using var stream = new MemoryStream();
        using (var writer = new Utf8JsonWriter(stream))
        {
            writer.WriteStartObject();
            foreach (var opt in def.Options)
            {
                writer.WritePropertyName(opt.Id);
                opt.DefaultValue.WriteTo(writer);
            }

            writer.WriteEndObject();
        }

        return JsonDocument.Parse(stream.ToArray()).RootElement.Clone();
    }

    private static JsonElement CloneOptions(JsonElement options, ServiceDefinition? def)
    {
        if (options.ValueKind is JsonValueKind.Object or JsonValueKind.Array) return options.Clone();
        return def is null ? JsonDocument.Parse("{}").RootElement.Clone() : DefaultOptions(def);
    }
}
