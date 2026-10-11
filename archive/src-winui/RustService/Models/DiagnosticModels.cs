namespace RustService.Models;

public sealed class NetworkInterfaceDetails
{
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public string? MacAddress { get; set; }
    public List<string> Ipv4Addresses { get; set; } = [];
    public List<string> Ipv6Addresses { get; set; } = [];
    public string? Gateway { get; set; }
    public List<string> DnsServers { get; set; } = [];
    public string Status { get; set; } = "";
    public long? SpeedMbps { get; set; }
    public string InterfaceType { get; set; } = "";
}

public sealed class PingReply
{
    public int Seq { get; set; }
    public int? Ttl { get; set; }
    public long? TimeMs { get; set; }
    public string Status { get; set; } = "";
}

public sealed class PingResult
{
    public string Host { get; set; } = "";
    public string? ResolvedIp { get; set; }
    public int PacketsSent { get; set; }
    public int PacketsReceived { get; set; }
    public double PacketLossPercent { get; set; }
    public long? MinMs { get; set; }
    public long? MaxMs { get; set; }
    public long? AvgMs { get; set; }
    public List<PingReply> Replies { get; set; } = [];
    public string? Error { get; set; }
}

public sealed class TracerouteHop
{
    public int HopNumber { get; set; }
    public string? Hostname { get; set; }
    public string? IpAddress { get; set; }
    public long? Rtt1Ms { get; set; }
    public long? Rtt2Ms { get; set; }
    public long? Rtt3Ms { get; set; }
    public bool TimedOut { get; set; }
}

public sealed class TracerouteResult
{
    public string Host { get; set; } = "";
    public string? ResolvedIp { get; set; }
    public List<TracerouteHop> Hops { get; set; } = [];
    public bool Completed { get; set; }
    public string? Error { get; set; }
}

public sealed class DnsRecord
{
    public string RecordType { get; set; } = "";
    public string Value { get; set; } = "";
    public int? Ttl { get; set; }
}

public sealed class DnsLookupResult
{
    public string Query { get; set; } = "";
    public string QueryType { get; set; } = "";
    public List<DnsRecord> Answers { get; set; } = [];
    public long ResponseTimeMs { get; set; }
    public string? ServerUsed { get; set; }
    public string? Error { get; set; }
}

public sealed class WifiInfo
{
    public bool Connected { get; set; }
    public string? Ssid { get; set; }
    public string? Bssid { get; set; }
    public int? SignalPercent { get; set; }
    public int? Channel { get; set; }
    public int? FrequencyMhz { get; set; }
    public string? RadioType { get; set; }
    public string? Authentication { get; set; }
    public double? ReceiveRateMbps { get; set; }
    public double? TransmitRateMbps { get; set; }
    public string? Error { get; set; }
}

public sealed class StartupItem
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Command { get; set; } = "";
    public string? Path { get; set; }
    public string Source { get; set; } = "";
    public string SourceLocation { get; set; } = "";
    public bool Enabled { get; set; }
    public string? Publisher { get; set; }
    public string? Description { get; set; }
    public string Impact { get; set; } = "unknown";
}

public sealed class EventLogSource
{
    public string Name { get; set; } = "";
    public string DisplayName { get; set; } = "";
    public long? RecordsCount { get; set; }
    public string LogType { get; set; } = "";
}

public sealed class EventLogEntry
{
    public int Id { get; set; }
    public long RecordId { get; set; }
    public string TimeCreated { get; set; } = "";
    public string Level { get; set; } = "";
    public string LevelDisplay { get; set; } = "";
    public string Source { get; set; } = "";
    public string ProviderName { get; set; } = "";
    public string Message { get; set; } = "";
    public string? TaskCategory { get; set; }
    public List<string> Keywords { get; set; } = [];
    public string Computer { get; set; } = "";
    public string? User { get; set; }
}

public sealed class EventLogStats
{
    public string LogName { get; set; } = "";
    public int Errors24h { get; set; }
    public int Warnings24h { get; set; }
    public int Errors7d { get; set; }
    public int Warnings7d { get; set; }
    public int Errors30d { get; set; }
    public int Warnings30d { get; set; }
    public int Critical24h { get; set; }
}

public sealed class BsodEntry
{
    public string Id { get; set; } = "";
    public string DumpPath { get; set; } = "";
    public string CrashTime { get; set; } = "";
    public string StopCode { get; set; } = "";
    public string? StopCodeName { get; set; }
    public string? FaultingModule { get; set; }
    public string BugCheckCode { get; set; } = "";
    public List<string> Parameters { get; set; } = [];
    public long FileSizeBytes { get; set; }
}

public sealed class BsodDetails
{
    public string DumpPath { get; set; } = "";
    public string DumpType { get; set; } = "";
    public string CrashTime { get; set; } = "";
    public string? UptimeBeforeCrash { get; set; }
    public string StopCode { get; set; } = "";
    public string? StopCodeName { get; set; }
    public string? StopCodeDescription { get; set; }
    public string BugCheckCode { get; set; } = "";
    public List<string> Parameters { get; set; } = [];
    public string? FaultingModule { get; set; }
    public string? FaultingModulePath { get; set; }
    public List<string> StackTrace { get; set; } = [];
    public List<string> LoadedModules { get; set; } = [];
    public List<string> PossibleCauses { get; set; } = [];
    public List<string> Recommendations { get; set; } = [];
}

public sealed class BsodStats
{
    public int TotalCrashes { get; set; }
    public int CrashesLast7Days { get; set; }
    public int CrashesLast30Days { get; set; }
    public string? MostCommonStopCode { get; set; }
    public string? MostCommonModule { get; set; }
    public string? OldestCrash { get; set; }
    public string? NewestCrash { get; set; }
}

public sealed class ShortcutDef
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public string Command { get; set; } = "";
    public string[]? Args { get; set; }
    public bool AdminRequired { get; set; }
}

public sealed class ShortcutGroup
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public List<ShortcutDef> Items { get; set; } = [];
}
