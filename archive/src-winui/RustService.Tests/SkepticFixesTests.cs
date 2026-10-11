using System.Text.Json;
using Microsoft.Win32;
using RustService.Core;
using RustService.Models;
using RustService.Services;
using Xunit;

namespace RustService.Tests;

public class DataDirectoryRootTests
{
    [Fact]
    public void Ensure_without_root_uses_base_directory_not_package_json_walk()
    {
        using var host = new TempDataHost();
        var previous = DataDirectory.Root;
        try
        {
            DataDirectory.Ensure();
            var expected = Path.GetFullPath(Path.Combine(
                AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar),
                "data"));
            Assert.Equal(expected, Path.GetFullPath(DataDirectory.Root));
            var repo = CommandMapTests.FindRepo();
            Assert.True(File.Exists(Path.Combine(repo, "package.json")));
            Assert.NotEqual(Path.GetFullPath(Path.Combine(repo, "data")), Path.GetFullPath(DataDirectory.Root));
            Assert.StartsWith(Path.GetFullPath(AppContext.BaseDirectory), Path.GetFullPath(DataDirectory.Root), StringComparison.OrdinalIgnoreCase);
        }
        finally
        {
            DataDirectory.Ensure(previous);
        }
    }
}

public class ProviderKeyTests
{
    [Fact]
    public void Api_key_is_stored_for_selected_provider_not_always_openai()
    {
        var keys = new ProviderApiKeys();
        keys.Set(AgentProvider.Anthropic, "sk-ant-test");
        keys.Set(AgentProvider.Xai, "xai-test");
        Assert.Equal("sk-ant-test", keys.Get(AgentProvider.Anthropic));
        Assert.Equal("xai-test", keys.Get(AgentProvider.Xai));
        Assert.Null(keys.Get(AgentProvider.OpenAI));
        Assert.Equal("sk-ant-test", AgentHost.KeyFor(new AgentSettings { Provider = AgentProvider.Anthropic, ApiKeys = keys }));
        Assert.Null(AgentHost.KeyFor(new AgentSettings { Provider = AgentProvider.OpenAI, ApiKeys = keys }));
    }
}

public class StartupApprovedTests
{
    [Fact]
    public void Registry_run_items_read_and_toggle_startup_approved_bytes()
    {
        var name = "RustServiceTestStartup_" + Guid.NewGuid().ToString("N")[..8];
        try
        {
            using (var run = Registry.CurrentUser.CreateSubKey(StartupService.RunPath))
            {
                run!.SetValue(name, "cmd.exe /c echo rustservice-test");
            }

            using (var approved = Registry.CurrentUser.CreateSubKey(StartupService.ApprovedRunPath))
            {
                approved!.SetValue(name, StartupService.DisabledBytes(), RegistryValueKind.Binary);
            }

            Assert.False(StartupService.IsApprovedEnabled(StartupService.DisabledBytes()));
            Assert.True(StartupService.IsApprovedEnabled(StartupService.EnabledBytes()));
            Assert.True(StartupService.IsApprovedEnabled(null));

            var service = new StartupService();
            var disabled = service.GetAll().FirstOrDefault(i => i.Name == name);
            Assert.NotNull(disabled);
            Assert.False(disabled!.Enabled);

            service.Toggle(disabled.Id, true);
            var enabled = service.GetAll().First(i => i.Name == name);
            Assert.True(enabled.Enabled);

            using var check = Registry.CurrentUser.OpenSubKey(StartupService.ApprovedRunPath);
            var bytes = check?.GetValue(name) as byte[];
            Assert.NotNull(bytes);
            Assert.True(StartupService.IsApprovedEnabled(bytes));

            service.Toggle(enabled.Id, false);
            Assert.False(service.GetAll().First(i => i.Name == name).Enabled);
        }
        finally
        {
            Registry.CurrentUser.OpenSubKey(StartupService.RunPath, true)?.DeleteValue(name, false);
            Registry.CurrentUser.OpenSubKey(StartupService.ApprovedRunPath, true)?.DeleteValue(name, false);
        }
    }
}

public class InstalledSoftwareRegistryTests
{
    [Fact]
    public async Task Installed_software_reads_uninstall_registry_not_win32_product()
    {
        using var host = new TempDataHost();
        var keyName = "RustServiceTestUninstall_" + Guid.NewGuid().ToString("N")[..8];
        var display = "RustService Test App " + keyName;
        using (var key = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\" + keyName))
        {
            key!.SetValue("DisplayName", display);
            key.SetValue("DisplayVersion", "9.9.9");
            key.SetValue("Publisher", "RustService");
            key.SetValue("EstimatedSize", 1024, RegistryValueKind.DWord);
        }

        try
        {
            var listed = SoftwareInventory.List(includeUpdates: true);
            var found = listed.FirstOrDefault(p => p.Name == display);
            Assert.NotNull(found);
            Assert.Equal("9.9.9", found!.Version);
            Assert.Equal("RustService", found.Publisher);

            var result = await ServiceJobs.RunAsync(
                "installed-software",
                TempDataHost.Json("{\"include_updates\":true}"),
                _ => { },
                CancellationToken.None);
            Assert.True(result.Success);
            Assert.Contains(display, string.Join("\n", result.Logs));
            Assert.DoesNotContain("Win32_Product", File.ReadAllText(
                Path.Combine(CommandMapTests.FindRepo(), "src-winui", "RustService", "Services", "ServiceJobs.cs")));
        }
        finally
        {
            Registry.CurrentUser.DeleteSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\" + keyName, throwOnMissingSubKey: false);
        }
    }
}

public class AgentListDirSourceTests
{
    [Fact]
    public void List_dir_handler_only_lists()
    {
        var src = File.ReadAllText(Path.Combine(CommandMapTests.FindRepo(), "src-winui", "RustService", "Views", "AgentPage.xaml.cs"));
        var start = src.IndexOf("private void ListDir_Click", StringComparison.Ordinal);
        Assert.True(start >= 0);
        var end = src.IndexOf("private void WriteFile_Click", start, StringComparison.Ordinal);
        var body = src[start..end];
        Assert.Contains("AgentListDir", body);
        Assert.DoesNotContain("GenerateAgentFile", body);
        Assert.DoesNotContain("SaveUploadedFile", body);
        Assert.DoesNotContain("AgentCopyFile", body);
        Assert.DoesNotContain("AgentMoveFile", body);
        Assert.DoesNotContain("DeleteAgentFile", body);
    }
}
