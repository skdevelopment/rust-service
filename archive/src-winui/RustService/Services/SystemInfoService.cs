using System.Diagnostics;
using System.Net.NetworkInformation;
using System.Runtime.InteropServices;
using RustService.Core;
using RustService.Models;

namespace RustService.Services;

public sealed class SystemInfoService
{
    public MachineSystemInfo Get()
    {
        var info = new MachineSystemInfo();
        FillOs(info);
        FillCpu(info);
        FillMemory(info);
        FillDisks(info);
        FillBoard(info);
        FillGpu(info);
        FillBattery(info);
        FillNetwork(info);
        FillUsers(info);
        FillProcesses(info);
        info.UptimeSeconds = (ulong)(Environment.TickCount64 / 1000);
        info.BootTime = (ulong)DateTimeOffset.UtcNow.ToUnixTimeSeconds() - info.UptimeSeconds;
        return info;
    }

    public IReadOnlyList<DiskHealthInfo> GetDiskHealth()
    {
        var list = new List<DiskHealthInfo>();
        try
        {
            foreach (var disk in Wmi.Query("SELECT Model, Status, Size, MediaType, DeviceID FROM Win32_DiskDrive"))
            {
                list.Add(new DiskHealthInfo
                {
                    Device = Wmi.Str(disk, "DeviceID") ?? "",
                    Model = Wmi.Str(disk, "Model") ?? "",
                    Status = Wmi.Str(disk, "Status") ?? "",
                    MediaType = Wmi.Str(disk, "MediaType") ?? "",
                    SizeBytes = Wmi.U64(disk, "Size")
                });
            }
        }
        catch (Exception ex)
        {
            list.Add(new DiskHealthInfo { Model = "Unavailable", Status = ex.Message });
        }

        return list;
    }

    public IReadOnlyList<RestorePointInfo> GetRestorePoints()
    {
        var list = new List<RestorePointInfo>();
        try
        {
            foreach (var point in Wmi.Query("SELECT SequenceNumber, Description, RestorePointType, CreationTime FROM SystemRestore", @"\\.\root\default"))
            {
                list.Add(new RestorePointInfo
                {
                    SequenceNumber = Wmi.U32(point, "SequenceNumber"),
                    Description = Wmi.Str(point, "Description") ?? "",
                    Type = Wmi.Str(point, "RestorePointType") ?? "",
                    CreationTime = Wmi.Str(point, "CreationTime") ?? ""
                });
            }
        }
        catch
        {
            // System Restore WMI may be unavailable
        }

        return list;
    }

    public async Task<string> CreateRestorePointAsync(string description)
    {
        var result = await ProcessRunner.RunAsync("powershell",
        [
            "-NoProfile", "-Command",
            $"Checkpoint-Computer -Description '{description.Replace("'", "''")}' -RestorePointType MODIFY_SETTINGS"
        ]);
        if (!result.Success) throw new InvalidOperationException(result.Combined.Trim());
        return result.Combined;
    }

    private static void FillOs(MachineSystemInfo info)
    {
        info.Os = new OsInfo
        {
            Name = RuntimeInformation.OSDescription,
            KernelVersion = Environment.OSVersion.Version.ToString(),
            OsVersion = Environment.OSVersion.VersionString,
            LongOsVersion = RuntimeInformation.OSDescription,
            Hostname = Environment.MachineName
        };

        try
        {
            var os = Wmi.Query("SELECT Caption, Version, BuildNumber FROM Win32_OperatingSystem").FirstOrDefault();
            if (os is not null)
            {
                info.Os.Name = Wmi.Str(os, "Caption");
                info.Os.OsVersion = Wmi.Str(os, "Version");
                info.Os.LongOsVersion = $"{Wmi.Str(os, "Caption")} {Wmi.Str(os, "Version")}";
            }
        }
        catch
        {
            // keep fallback
        }
    }

    private static void FillCpu(MachineSystemInfo info)
    {
        try
        {
            var cpus = Wmi.Query("SELECT Name, Manufacturer, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed, L2CacheSize, L3CacheSize, SocketDesignation FROM Win32_Processor").ToList();
            var first = cpus.FirstOrDefault();
            var logical = (uint)Environment.ProcessorCount;
            info.Cpu = new CpuInfo
            {
                Brand = first is null ? "" : Wmi.Str(first, "Name") ?? "",
                VendorId = first is null ? "" : Wmi.Str(first, "Manufacturer") ?? "",
                PhysicalCores = first is null ? null : Wmi.U32(first, "NumberOfCores"),
                LogicalCpus = first is null ? logical : Wmi.U32(first, "NumberOfLogicalProcessors"),
                FrequencyMhz = first is null ? 0 : Wmi.U32(first, "MaxClockSpeed"),
                GlobalUsage = GetCpuUsage()
            };
            if (first is not null)
            {
                info.CpuL2CacheKb = Wmi.U32(first, "L2CacheSize");
                info.CpuL3CacheKb = Wmi.U32(first, "L3CacheSize");
                info.CpuSocket = Wmi.Str(first, "SocketDesignation");
            }

            info.Cpu.Cores = Enumerable.Range(0, (int)info.Cpu.LogicalCpus)
                .Select(i => new CpuCoreInfo { Name = $"CPU {i}", CpuUsage = info.Cpu.GlobalUsage, FrequencyMhz = info.Cpu.FrequencyMhz })
                .ToList();
        }
        catch
        {
            info.Cpu.LogicalCpus = (uint)Environment.ProcessorCount;
        }
    }

    private static void FillMemory(MachineSystemInfo info)
    {
        try
        {
            var os = Wmi.Query("SELECT TotalVisibleMemorySize, FreePhysicalMemory, TotalVirtualMemorySize, FreeVirtualMemory FROM Win32_OperatingSystem").FirstOrDefault();
            if (os is null) return;
            var total = Wmi.U64(os, "TotalVisibleMemorySize") * 1024;
            var free = Wmi.U64(os, "FreePhysicalMemory") * 1024;
            var totalVirt = Wmi.U64(os, "TotalVirtualMemorySize") * 1024;
            var freeVirt = Wmi.U64(os, "FreeVirtualMemory") * 1024;
            info.Memory = new MemoryInfo
            {
                TotalMemory = total,
                AvailableMemory = free,
                UsedMemory = total - free,
                TotalSwap = totalVirt > total ? totalVirt - total : 0,
                UsedSwap = totalVirt > freeVirt ? totalVirt - freeVirt - (total - free) : 0
            };
        }
        catch
        {
            // ignore
        }

        try
        {
            foreach (var slot in Wmi.Query("SELECT BankLabel, DeviceLocator, Manufacturer, PartNumber, SerialNumber, Speed, Capacity, FormFactor, MemoryType FROM Win32_PhysicalMemory"))
            {
                info.RamSlots.Add(new RamSlotInfo
                {
                    BankLabel = Wmi.Str(slot, "BankLabel"),
                    DeviceLocator = Wmi.Str(slot, "DeviceLocator"),
                    Manufacturer = Wmi.Str(slot, "Manufacturer"),
                    PartNumber = Wmi.Str(slot, "PartNumber"),
                    SerialNumber = Wmi.Str(slot, "SerialNumber"),
                    SpeedMhz = Wmi.U32(slot, "Speed"),
                    CapacityBytes = Wmi.U64(slot, "Capacity"),
                    FormFactor = Wmi.Str(slot, "FormFactor"),
                    MemoryType = Wmi.Str(slot, "MemoryType")
                });
            }
        }
        catch
        {
            // ignore
        }
    }

    private static void FillDisks(MachineSystemInfo info)
    {
        foreach (var drive in DriveInfo.GetDrives().Where(d => d.IsReady))
        {
            info.Disks.Add(new DiskInfo
            {
                Name = drive.VolumeLabel,
                MountPoint = drive.Name,
                TotalSpace = (ulong)drive.TotalSize,
                AvailableSpace = (ulong)drive.AvailableFreeSpace,
                FileSystem = drive.DriveFormat,
                DiskType = drive.DriveType.ToString(),
                IsRemovable = drive.DriveType is DriveType.Removable
            });
        }
    }

    private static void FillBoard(MachineSystemInfo info)
    {
        try
        {
            var board = Wmi.Query("SELECT Product, Manufacturer, Version, SerialNumber FROM Win32_BaseBoard").FirstOrDefault();
            if (board is not null)
            {
                info.Motherboard = new MotherboardInfo
                {
                    Name = Wmi.Str(board, "Product"),
                    Vendor = Wmi.Str(board, "Manufacturer"),
                    Version = Wmi.Str(board, "Version"),
                    SerialNumber = Wmi.Str(board, "SerialNumber")
                };
            }

            var bios = Wmi.Query("SELECT Manufacturer, SMBIOSBIOSVersion, ReleaseDate, SerialNumber FROM Win32_BIOS").FirstOrDefault();
            if (bios is not null)
            {
                info.Bios = new BiosInfo
                {
                    Manufacturer = Wmi.Str(bios, "Manufacturer"),
                    Version = Wmi.Str(bios, "SMBIOSBIOSVersion"),
                    ReleaseDate = Wmi.Str(bios, "ReleaseDate"),
                    SerialNumber = Wmi.Str(bios, "SerialNumber")
                };
            }

            var product = Wmi.Query("SELECT Vendor, Name, IdentifyingNumber, UUID FROM Win32_ComputerSystemProduct").FirstOrDefault();
            if (product is not null)
            {
                info.SystemProduct = new SystemProductInfo
                {
                    Vendor = Wmi.Str(product, "Vendor"),
                    Model = Wmi.Str(product, "Name"),
                    SerialNumber = Wmi.Str(product, "IdentifyingNumber"),
                    Uuid = Wmi.Str(product, "UUID")
                };
            }
        }
        catch
        {
            // ignore
        }
    }

    private static void FillGpu(MachineSystemInfo info)
    {
        try
        {
            var gpu = Wmi.Query("SELECT Name, AdapterCompatibility, AdapterRAM, PNPDeviceID, DriverVersion FROM Win32_VideoController").FirstOrDefault();
            if (gpu is null) return;
            info.Gpu = new GpuInfo
            {
                Model = Wmi.Str(gpu, "Name") ?? "",
                Vendor = Wmi.Str(gpu, "AdapterCompatibility") ?? "",
                Family = Wmi.Str(gpu, "DriverVersion") ?? "",
                DeviceId = Wmi.Str(gpu, "PNPDeviceID") ?? "",
                TotalVram = Wmi.U64(gpu, "AdapterRAM")
            };
        }
        catch
        {
            // ignore
        }
    }

    private static void FillBattery(MachineSystemInfo info)
    {
        try
        {
            foreach (var bat in Wmi.Query("SELECT EstimatedChargeRemaining, DesignCapacity, FullChargeCapacity, Status, Name FROM Win32_Battery"))
            {
                var full = Wmi.U32(bat, "FullChargeCapacity");
                var design = Wmi.U32(bat, "DesignCapacity");
                info.Batteries.Add(new BatteryInfo
                {
                    StateOfCharge = Wmi.U32(bat, "EstimatedChargeRemaining"),
                    EnergyFullWh = full / 1000f,
                    EnergyFullDesignWh = design / 1000f,
                    StateOfHealth = design == 0 ? 0 : full * 100f / design,
                    State = Wmi.Str(bat, "Status") ?? "",
                    Model = Wmi.Str(bat, "Name")
                });
            }
        }
        catch
        {
            // ignore
        }
    }

    private static void FillNetwork(MachineSystemInfo info)
    {
        foreach (var nic in NetworkInterface.GetAllNetworkInterfaces())
        {
            var stats = nic.GetIPv4Statistics();
            info.Networks.Add(new NetworkInfo
            {
                Name = nic.Name,
                MacAddress = nic.GetPhysicalAddress().ToString(),
                TotalReceived = (ulong)stats.BytesReceived,
                TotalTransmitted = (ulong)stats.BytesSent,
                PacketsReceived = (ulong)stats.UnicastPacketsReceived,
                PacketsTransmitted = (ulong)stats.UnicastPacketsSent,
                ErrorsReceived = (ulong)stats.IncomingPacketsDiscarded,
                ErrorsTransmitted = (ulong)stats.OutgoingPacketsDiscarded
            });
        }
    }

    private static void FillUsers(MachineSystemInfo info)
    {
        info.Users.Add(new UserInfo { Name = Environment.UserName, Groups = [Environment.UserDomainName] });
    }

    private static void FillProcesses(MachineSystemInfo info)
    {
        info.TopProcesses = Process.GetProcesses()
            .OrderByDescending(p =>
            {
                try { return p.WorkingSet64; }
                catch { return 0L; }
            })
            .Take(15)
            .Select(p =>
            {
                try
                {
                    return new ProcessInfo
                    {
                        Pid = (uint)p.Id,
                        Name = p.ProcessName,
                        MemoryBytes = (ulong)Math.Max(0, p.WorkingSet64)
                    };
                }
                catch
                {
                    return new ProcessInfo { Name = p.ProcessName };
                }
            })
            .ToList();
    }

    private static float GetCpuUsage()
    {
        try
        {
            using var counter = new PerformanceCounter("Processor", "% Processor Time", "_Total");
            counter.NextValue();
            Thread.Sleep(200);
            return counter.NextValue();
        }
        catch
        {
            return 0;
        }
    }
}
