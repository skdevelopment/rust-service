using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using RustService.Services;

namespace RustService.Views;

public sealed partial class SystemInfoPage : Page
{
    public SystemInfoPage()
    {
        InitializeComponent();
        Loaded += (_, _) => Refresh();
    }

    private void Refresh_Click(object sender, RoutedEventArgs e) => Refresh();

    private async void Restore_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            StatusText.Text = await Commands.CreateRestorePoint("RustService");
            Refresh();
        }
        catch (Exception ex)
        {
            StatusText.Text = ex.Message;
        }
    }

    private void Refresh()
    {
        var info = Commands.GetSystemInfo();
        var fp = Commands.GetPcFingerprint();
        var cards = new List<CardRow>
        {
            new() { Title = "Operating system", Subtitle = $"{info.Os.Name}\n{info.Os.Hostname}", Badge = "OS" },
            new() { Title = "CPU", Subtitle = $"{info.Cpu.Brand}\n{info.Cpu.PhysicalCores}c/{info.Cpu.LogicalCpus}t  {info.Cpu.GlobalUsage:0.0}%", Badge = "CPU" },
            new() { Title = "Memory", Subtitle = $"{info.Memory.UsedMemory / 1_000_000} / {info.Memory.TotalMemory / 1_000_000} MB", Badge = "RAM" },
            new() { Title = "Fingerprint", Subtitle = $"{fp.MachineName}\ncpuScore={fp.CpuScore:0.0}  RAM={fp.TotalRamGb:0.0} GB", Badge = "ID" }
        };
        if (info.Gpu is not null)
        {
            cards.Add(new CardRow { Title = "GPU", Subtitle = $"{info.Gpu.Vendor} {info.Gpu.Model}", Badge = "GPU" });
        }
        if (info.Motherboard is not null)
        {
            cards.Add(new CardRow { Title = "Board", Subtitle = $"{info.Motherboard.Vendor} {info.Motherboard.Name}", Badge = "Board" });
        }
        foreach (var disk in info.Disks)
        {
            cards.Add(new CardRow
            {
                Title = $"Disk {disk.MountPoint}",
                Subtitle = $"{disk.AvailableSpace / 1_000_000_000} GB free / {disk.TotalSpace / 1_000_000_000} GB",
                Badge = disk.DiskType
            });
        }
        foreach (var point in Commands.GetRestorePoints())
        {
            cards.Add(new CardRow { Title = "Restore point", Subtitle = $"{point.SequenceNumber} {point.Description} {point.CreationTime}", Badge = "Restore" });
        }
        foreach (var disk in Commands.GetDiskHealth())
        {
            cards.Add(new CardRow { Title = "Disk health", Subtitle = $"{disk.Model} {disk.Status} {disk.MediaType}", Badge = disk.Status });
        }
        Cards.ItemsSource = cards;
        StatusText.Text = $"{cards.Count} cards";
    }
}
