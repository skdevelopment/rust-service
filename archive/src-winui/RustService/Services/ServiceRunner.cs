using System.Text.Json;
using RustService.Models;

namespace RustService.Services;

public sealed class ServiceRunner
{
    private readonly object sync = new();
    private readonly Func<string, JsonElement, Action<string>, CancellationToken, Task<ServiceResult>> runJob;
    private CancellationTokenSource? cts;
    private TaskCompletionSource? pauseGate;
    public ExclusiveResourceLock ResourceLock { get; } = new();
    public ServiceRunState State { get; } = new();
    public List<(string ServiceId, DateTime Start, DateTime End, string[] Resources)> ExecutionLog { get; } = [];
    public event Action<ServiceRunState>? StateChanged;
    public event Action<string, string>? LogReceived;

    public ServiceRunner(Func<string, JsonElement, Action<string>, CancellationToken, Task<ServiceResult>>? runJob = null)
    {
        this.runJob = runJob ?? ServiceJobs.RunAsync;
    }

    public async Task<ServiceReport> RunAsync(
        IReadOnlyList<ServiceQueueItem> queue,
        bool parallel,
        string? technician = null,
        string? customer = null)
    {
        lock (sync)
        {
            if (State.IsRunning) throw new InvalidOperationException("A service run is already in progress.");
            cts = new CancellationTokenSource();
            ExecutionLog.Clear();
            State.IsRunning = true;
            State.IsPaused = false;
            State.CurrentReport = new ServiceReport
            {
                Queue = queue.ToList(),
                Status = ServiceRunStatus.Running,
                ParallelMode = parallel,
                TechnicianName = technician,
                CustomerName = customer
            };
        }

        Notify();
        var token = cts!.Token;
        var enabled = queue.Where(q => q.Enabled).OrderBy(q => q.Order).ToList();
        try
        {
            if (parallel) await RunParallel(enabled, token);
            else await RunSequential(enabled, token);

            lock (sync)
            {
                State.CurrentReport!.Status = token.IsCancellationRequested
                    ? ServiceRunStatus.Cancelled
                    : State.CurrentReport.Results.Any(r => !r.Success) ? ServiceRunStatus.Failed : ServiceRunStatus.Completed;
                State.CurrentReport.CompletedAt = DateTime.UtcNow.ToString("O");
                State.CurrentReport.TotalDurationMs = State.CurrentReport.Results.Sum(r => r.DurationMs);
                State.CurrentReport.HealthScore = AppServices.Reports.Statistics(State.CurrentReport).HealthScore;
            }
        }
        catch (OperationCanceledException)
        {
            lock (sync)
            {
                if (State.CurrentReport is not null)
                {
                    State.CurrentReport.Status = ServiceRunStatus.Cancelled;
                    State.CurrentReport.CompletedAt = DateTime.UtcNow.ToString("O");
                }
            }
        }
        finally
        {
            ResourceLock.Clear();
            lock (sync)
            {
                State.IsRunning = false;
                State.IsPaused = false;
                cts?.Dispose();
                cts = null;
            }

            if (State.CurrentReport is not null)
            {
                AppServices.Reports.Save(State.CurrentReport);
            }

            Notify();
        }

        return State.CurrentReport!;
    }

    public void Cancel() => cts?.Cancel();

    public void Pause()
    {
        lock (sync)
        {
            if (!State.IsRunning || State.IsPaused || State.CurrentReport?.ParallelMode == true) return;
            State.IsPaused = true;
            pauseGate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        }

        Notify();
    }

    public void Resume()
    {
        lock (sync)
        {
            State.IsPaused = false;
            pauseGate?.TrySetResult();
            pauseGate = null;
        }

        Notify();
    }

    private async Task RunSequential(List<ServiceQueueItem> items, CancellationToken token)
    {
        for (var i = 0; i < items.Count; i++)
        {
            token.ThrowIfCancellationRequested();
            await WaitIfPaused();
            lock (sync)
            {
                State.CurrentReport!.CurrentServiceIndex = i;
                State.CurrentReport.CurrentServiceIndices = [i];
            }

            Notify();
            var result = await Execute(items[i], token);
            lock (sync) State.CurrentReport!.Results.Add(result);
            Notify();
        }
    }

    private async Task RunParallel(List<ServiceQueueItem> items, CancellationToken token)
    {
        var remaining = new List<ServiceQueueItem>(items);
        var running = new List<Task<(int Index, ServiceResult Result, string[] Resources)>>();

        while (remaining.Count > 0 || running.Count > 0)
        {
            if (!token.IsCancellationRequested)
            {
                for (var i = remaining.Count - 1; i >= 0; i--)
                {
                    var item = remaining[i];
                    var resources = ServiceCatalog.Get(item.ServiceId)?.ExclusiveResources.ToArray() ?? [];
                    if (!ResourceLock.TryAcquire(resources)) continue;
                    remaining.RemoveAt(i);
                    var index = items.IndexOf(item);
                    lock (sync)
                    {
                        if (!State.CurrentReport!.CurrentServiceIndices.Contains(index))
                        {
                            State.CurrentReport.CurrentServiceIndices.Add(index);
                        }
                    }

                    Notify();
                    running.Add(RunOne(item, index, resources, token));
                }
            }
            else
            {
                remaining.Clear();
            }

            if (running.Count == 0)
            {
                if (remaining.Count == 0 || token.IsCancellationRequested) break;
                ResourceLock.Clear();
                continue;
            }

            var finished = await Task.WhenAny(running);
            running.Remove(finished);
            var (indexDone, result, released) = await finished;
            ResourceLock.Release(released);
            lock (sync)
            {
                State.CurrentReport!.Results.Add(result);
                State.CurrentReport.CurrentServiceIndices.Remove(indexDone);
            }

            Notify();
        }
    }

    private async Task<(int Index, ServiceResult Result, string[] Resources)> RunOne(
        ServiceQueueItem item, int index, string[] resources, CancellationToken token)
    {
        try
        {
            var result = await Execute(item, token);
            return (index, result, resources);
        }
        catch (OperationCanceledException)
        {
            return (index, new ServiceResult { ServiceId = item.ServiceId, Success = false, Error = "Cancelled" }, resources);
        }
    }

    private async Task<ServiceResult> Execute(ServiceQueueItem item, CancellationToken token)
    {
        var missing = AppServices.RequiredPrograms.Validate(ServiceCatalog.Get(item.ServiceId)?.RequiredPrograms ?? []);
        if (missing.Count > 0)
        {
            return new ServiceResult
            {
                ServiceId = item.ServiceId,
                Success = false,
                Error = $"Missing programs: {string.Join(", ", missing)}",
                Findings = [new ServiceFinding { Severity = FindingSeverity.Error, Title = "Missing requirement", Description = string.Join(", ", missing) }]
            };
        }

        var options = item.Options.ValueKind is JsonValueKind.Undefined or JsonValueKind.Null
            ? JsonDocument.Parse("{}").RootElement
            : item.Options;
        var started = DateTime.UtcNow;
        var resources = ServiceCatalog.Get(item.ServiceId)?.ExclusiveResources.ToArray() ?? [];
        var result = await runJob(item.ServiceId, options, line => LogReceived?.Invoke(item.ServiceId, line), token);
        var ended = DateTime.UtcNow;
        lock (sync)
        {
            ExecutionLog.Add((item.ServiceId, started, ended, resources));
        }

        result.ServiceId = item.ServiceId;
        Commands.RecordServiceTime(item.ServiceId, result.DurationMs);
        return result;
    }

    private async Task WaitIfPaused()
    {
        TaskCompletionSource? gate;
        lock (sync) gate = pauseGate;
        if (gate is not null) await gate.Task;
    }

    private void Notify() => StateChanged?.Invoke(State);
}
