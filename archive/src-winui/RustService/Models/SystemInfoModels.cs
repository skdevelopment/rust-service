namespace RustService.Models;

public sealed class OsInfo
{
    public string? Name { get; set; }
    public string? KernelVersion { get; set; }
    public string? OsVersion { get; set; }
    public string? LongOsVersion { get; set; }
    public string? Hostname { get; set; }
}

public sealed class CpuCoreInfo
{
    public string Name { get; set; } = "";
    public float CpuUsage { get; set; }
    public ulong FrequencyMhz { get; set; }
}

public sealed class CpuInfo
{
    public string Brand { get; set; } = "";
    public string VendorId { get; set; } = "";
    public uint? PhysicalCores { get; set; }
    public uint LogicalCpus { get; set; }
    public ulong FrequencyMhz { get; set; }
    public float GlobalUsage { get; set; }
    public List<CpuCoreInfo> Cores { get; set; } = [];
}

public sealed class MemoryInfo
{
    public ulong TotalMemory { get; set; }
    public ulong UsedMemory { get; set; }
    public ulong AvailableMemory { get; set; }
    public ulong TotalSwap { get; set; }
    public ulong UsedSwap { get; set; }
}

public sealed class DiskInfo
{
    public string Name { get; set; } = "";
    public string MountPoint { get; set; } = "";
    public ulong TotalSpace { get; set; }
    public ulong AvailableSpace { get; set; }
    public string FileSystem { get; set; } = "";
    public string DiskType { get; set; } = "";
    public bool IsRemovable { get; set; }
}

public sealed class MotherboardInfo
{
    public string? Name { get; set; }
    public string? Vendor { get; set; }
    public string? Version { get; set; }
    public string? SerialNumber { get; set; }
}

public sealed class GpuInfo
{
    public string Vendor { get; set; } = "";
    public string Model { get; set; } = "";
    public string Family { get; set; } = "";
    public string DeviceId { get; set; } = "";
    public ulong TotalVram { get; set; }
    public ulong UsedVram { get; set; }
    public float LoadPct { get; set; }
    public uint Temperature { get; set; }
}

public sealed class BatteryInfo
{
    public float StateOfCharge { get; set; }
    public float EnergyWh { get; set; }
    public float EnergyFullWh { get; set; }
    public float EnergyFullDesignWh { get; set; }
    public float PowerRateW { get; set; }
    public float Voltage { get; set; }
    public float StateOfHealth { get; set; }
    public string State { get; set; } = "";
    public string Technology { get; set; } = "";
    public float? Temperature { get; set; }
    public uint? CycleCount { get; set; }
    public string? Vendor { get; set; }
    public string? Model { get; set; }
    public uint? TimeToFullSecs { get; set; }
    public uint? TimeToEmptySecs { get; set; }
}

public sealed class ComponentInfo
{
    public string Label { get; set; } = "";
    public float? Temperature { get; set; }
    public float? MaxTemperature { get; set; }
    public float? CriticalTemperature { get; set; }
    public string? Id { get; set; }
}

public sealed class LoadAvgInfo
{
    public double One { get; set; }
    public double Five { get; set; }
    public double Fifteen { get; set; }
}

public sealed class NetworkInfo
{
    public string Name { get; set; } = "";
    public string MacAddress { get; set; } = "";
    public ulong Received { get; set; }
    public ulong TotalReceived { get; set; }
    public ulong Transmitted { get; set; }
    public ulong TotalTransmitted { get; set; }
    public ulong PacketsReceived { get; set; }
    public ulong PacketsTransmitted { get; set; }
    public ulong ErrorsReceived { get; set; }
    public ulong ErrorsTransmitted { get; set; }
}

public sealed class UserInfo
{
    public string Name { get; set; } = "";
    public List<string> Groups { get; set; } = [];
}

public sealed class ProcessInfo
{
    public uint Pid { get; set; }
    public string Name { get; set; } = "";
    public float CpuUsage { get; set; }
    public ulong MemoryBytes { get; set; }
}

public sealed class BiosInfo
{
    public string? Manufacturer { get; set; }
    public string? Version { get; set; }
    public string? ReleaseDate { get; set; }
    public string? SerialNumber { get; set; }
}

public sealed class SystemProductInfo
{
    public string? Vendor { get; set; }
    public string? Model { get; set; }
    public string? SerialNumber { get; set; }
    public string? Uuid { get; set; }
}

public sealed class RamSlotInfo
{
    public string? BankLabel { get; set; }
    public string? DeviceLocator { get; set; }
    public string? Manufacturer { get; set; }
    public string? PartNumber { get; set; }
    public string? SerialNumber { get; set; }
    public uint? SpeedMhz { get; set; }
    public ulong? CapacityBytes { get; set; }
    public string? FormFactor { get; set; }
    public string? MemoryType { get; set; }
}

public sealed class MachineSystemInfo
{
    public OsInfo Os { get; set; } = new();
    public CpuInfo Cpu { get; set; } = new();
    public MemoryInfo Memory { get; set; } = new();
    public List<DiskInfo> Disks { get; set; } = [];
    public MotherboardInfo? Motherboard { get; set; }
    public GpuInfo? Gpu { get; set; }
    public List<BatteryInfo> Batteries { get; set; } = [];
    public List<ComponentInfo> Components { get; set; } = [];
    public LoadAvgInfo LoadAvg { get; set; } = new();
    public List<NetworkInfo> Networks { get; set; } = [];
    public List<UserInfo> Users { get; set; } = [];
    public List<ProcessInfo> TopProcesses { get; set; } = [];
    public ulong UptimeSeconds { get; set; }
    public ulong BootTime { get; set; }
    public BiosInfo? Bios { get; set; }
    public SystemProductInfo? SystemProduct { get; set; }
    public List<RamSlotInfo> RamSlots { get; set; } = [];
    public ulong? CpuL2CacheKb { get; set; }
    public ulong? CpuL3CacheKb { get; set; }
    public string? CpuSocket { get; set; }
}

public sealed class DiskHealthInfo
{
    public string Device { get; set; } = "";
    public string Model { get; set; } = "";
    public string Status { get; set; } = "";
    public string MediaType { get; set; } = "";
    public ulong SizeBytes { get; set; }
}

public sealed class RestorePointInfo
{
    public uint SequenceNumber { get; set; }
    public string Description { get; set; } = "";
    public string Type { get; set; } = "";
    public string CreationTime { get; set; } = "";
}
