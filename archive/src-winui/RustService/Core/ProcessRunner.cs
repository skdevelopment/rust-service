using System.Diagnostics;
using System.Text;

namespace RustService.Core;

public sealed record ProcessResult(int ExitCode, string Stdout, string Stderr)
{
    public bool Success => ExitCode == 0;
    public string Combined => string.IsNullOrWhiteSpace(Stderr) ? Stdout : $"{Stdout}\n{Stderr}";
}

public static class ProcessRunner
{
    public static async Task<ProcessResult> RunAsync(
        string fileName,
        IEnumerable<string>? args = null,
        string? workingDirectory = null,
        int timeoutMs = 0,
        Action<string>? onLine = null,
        CancellationToken cancellationToken = default)
    {
        var psi = new ProcessStartInfo
        {
            FileName = fileName,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            WorkingDirectory = workingDirectory ?? Environment.CurrentDirectory
        };

        if (args != null)
        {
            foreach (var arg in args)
            {
                psi.ArgumentList.Add(arg);
            }
        }

        using var process = new Process { StartInfo = psi, EnableRaisingEvents = true };
        var stdout = new StringBuilder();
        var stderr = new StringBuilder();
        process.OutputDataReceived += (_, e) =>
        {
            if (e.Data is null) return;
            stdout.AppendLine(e.Data);
            onLine?.Invoke(e.Data);
        };
        process.ErrorDataReceived += (_, e) =>
        {
            if (e.Data is null) return;
            stderr.AppendLine(e.Data);
            onLine?.Invoke(e.Data);
        };

        process.Start();
        process.BeginOutputReadLine();
        process.BeginErrorReadLine();

        using var timeoutCts = timeoutMs > 0 ? new CancellationTokenSource(timeoutMs) : null;
        using var linked = timeoutCts is null
            ? CancellationTokenSource.CreateLinkedTokenSource(cancellationToken)
            : CancellationTokenSource.CreateLinkedTokenSource(cancellationToken, timeoutCts.Token);

        try
        {
            await process.WaitForExitAsync(linked.Token);
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            TryKill(process);
            return new ProcessResult(-1, stdout.ToString(), "Timed out");
        }
        catch (OperationCanceledException)
        {
            TryKill(process);
            throw;
        }

        return new ProcessResult(process.ExitCode, stdout.ToString(), stderr.ToString());
    }

    public static void StartDetached(string fileName, IEnumerable<string>? args = null, bool admin = false)
    {
        if (admin)
        {
            var argumentList = args is null ? "" : string.Join(",", args.Select(a => $"'{a.Replace("'", "''")}'"));
            var extra = string.IsNullOrEmpty(argumentList) ? "" : $" -ArgumentList {argumentList}";
            Process.Start(new ProcessStartInfo
            {
                FileName = "powershell",
                Arguments = $"-Command Start-Process -FilePath '{fileName.Replace("'", "''")}' -Verb RunAs{extra}",
                UseShellExecute = false,
                CreateNoWindow = true
            });
            return;
        }

        string exe = fileName;
        var finalArgs = args?.ToList() ?? [];
        if (fileName.EndsWith(".msc", StringComparison.OrdinalIgnoreCase))
        {
            exe = "mmc.exe";
            finalArgs = [fileName, .. finalArgs];
        }
        else if (fileName.EndsWith(".cpl", StringComparison.OrdinalIgnoreCase))
        {
            exe = "control.exe";
            finalArgs = [fileName, .. finalArgs];
        }

        var info = new ProcessStartInfo { FileName = exe, UseShellExecute = true };
        foreach (var arg in finalArgs)
        {
            info.ArgumentList.Add(arg);
        }

        Process.Start(info);
    }

    public static void OpenFolder(string path)
    {
        Process.Start(new ProcessStartInfo
        {
            FileName = "explorer.exe",
            Arguments = Directory.Exists(path) ? path : $"/select,\"{path}\"",
            UseShellExecute = true
        });
    }

    private static void TryKill(Process process)
    {
        try
        {
            if (!process.HasExited) process.Kill(true);
        }
        catch
        {
            // ignored
        }
    }
}
