using System.IO;
using Vajra.Windows.Models;
using Vajra.Windows.Services;
using Vajra.Windows.ViewModels;
using Xunit;

namespace Vajra.Windows.Tests;

public class MockDaemonClientForSync : IDaemonClient
{
    public List<DownloadInfoResponse> SnapshotDownloads { get; set; } = new();
    public int GetDownloadsCallCount { get; private set; }
    public Func<Task>? OnGetDownloadsAsync { get; set; }

    public string? LastError => null;

    public Task<HealthResponse?> CheckHealthAsync(CancellationToken ct = default)
    {
        return Task.FromResult<HealthResponse?>(new HealthResponse { Status = "ok", DaemonVersion = "1.0", ApiVersion = "1" });
    }

    public async Task<List<DownloadInfoResponse>> GetDownloadsAsync(CancellationToken ct = default)
    {
        GetDownloadsCallCount++;
        if (OnGetDownloadsAsync != null)
        {
            await OnGetDownloadsAsync();
        }
        return SnapshotDownloads.Select(d => new DownloadInfoResponse
        {
            Id = d.Id,
            Url = d.Url,
            Filename = d.Filename,
            OutputPath = d.OutputPath,
            Status = d.Status,
            TotalBytes = d.TotalBytes,
            BytesDone = d.BytesDone,
            SpeedBps = d.SpeedBps,
            EtaSeconds = d.EtaSeconds,
            ProgressPct = d.ProgressPct,
            CreatedAt = d.CreatedAt,
            Error = d.Error
        }).ToList();
    }

    public string? LastDeletedId { get; private set; }
    public bool? LastDeletedFileParam { get; private set; }
    public int DeleteCallCount { get; private set; }

    public Task<AddDownloadResponse?> AddDownloadAsync(string url, string? filename = null, string? outputDir = null, CancellationToken ct = default) => Task.FromResult<AddDownloadResponse?>(null);
    public Task<bool> PauseDownloadAsync(string id, CancellationToken ct = default) => Task.FromResult(true);
    public Task<bool> ResumeDownloadAsync(string id, CancellationToken ct = default) => Task.FromResult(true);
    public Task<bool> DeleteDownloadAsync(string id, bool deleteFile = false, CancellationToken ct = default)
    {
        LastDeletedId = id;
        LastDeletedFileParam = deleteFile;
        DeleteCallCount++;
        return Task.FromResult(true);
    }

    public BulkActionRequest? LastBulkActionRequest { get; private set; }
    public int BulkActionCallCount { get; private set; }
    public Func<BulkActionRequest, Task<BulkActionResponse?>>? BulkActionAsyncCallback { get; set; }
    public Task<BulkActionResponse?> BulkActionAsync(BulkActionRequest request, CancellationToken ct = default)
    {
        LastBulkActionRequest = request;
        BulkActionCallCount++;
        if (BulkActionAsyncCallback != null) return BulkActionAsyncCallback(request);
        return Task.FromResult<BulkActionResponse?>(new BulkActionResponse { Total = request.Ids.Count, Succeeded = new List<string>(request.Ids) });
    }
    public Task<bool> PauseAllAsync(CancellationToken ct = default) => Task.FromResult(true);
    public Task<bool> ResumeAllAsync(CancellationToken ct = default) => Task.FromResult(true);
    public Func<Task<BulkActionResponse?>>? OnClearCompletedAsync { get; set; }
    public Task<BulkActionResponse?> ClearCompletedAsync(CancellationToken ct = default)
    {
        if (OnClearCompletedAsync != null) return OnClearCompletedAsync();
        return Task.FromResult<BulkActionResponse?>(new BulkActionResponse { Total = 0, Succeeded = new List<string>() });
    }
    public string? LastRetriedId { get; private set; }
    public Task<bool> RetryDownloadAsync(string id, CancellationToken ct = default)
    {
        LastRetriedId = id;
        return Task.FromResult(true);
    }
    public Task<DownloadInfoResponse?> GetDownloadDetailsAsync(string id, CancellationToken ct = default) => Task.FromResult<DownloadInfoResponse?>(null);
}

public class MockSseServiceForSync : ISseService
{
#pragma warning disable CS0067
    public event Action<SseProgressPayload>? OnProgress;
    public event Action<SseBatchProgressPayload>? OnBatchProgress;
    public event Action<SseStateChangePayload>? OnStateChange;
    public event Action<SseAddedPayload>? OnAdded;
    public event Action<SseRemovedPayload>? OnRemoved;
    public event Action? OnConnected;
    public event Action? OnDisconnected;
    public event Action? OnAuthenticationFailed;
#pragma warning restore CS0067

    public bool IsConnected { get; set; }

    public Task StartAsync(CancellationToken ct = default)
    {
        IsConnected = true;
        OnConnected?.Invoke();
        return Task.CompletedTask;
    }

    public Task StopAsync()
    {
        IsConnected = false;
        OnDisconnected?.Invoke();
        return Task.CompletedTask;
    }

    public void RaiseProgress(SseProgressPayload payload) => OnProgress?.Invoke(payload);
    public void RaiseAdded(SseAddedPayload payload) => OnAdded?.Invoke(payload);
    public void RaiseStateChange(SseStateChangePayload payload) => OnStateChange?.Invoke(payload);
    public void RaiseAuthFailed() => OnAuthenticationFailed?.Invoke();

    public void Dispose() { }
}

public class MockSystemInteractionService : ISystemInteractionService
{
    public bool OpenFile(string path) => true;
    public bool ShowInFolder(string path) => true;
    public void CopyToClipboard(string text) { }
    public string? GetClipboardText() => null;
}

public class MainViewModelTests
{
    [Fact]
    public async Task ReconcileDownloads_DoesNotOverwriteFresherSseProgress_WithOlderSnapshot()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);

        // Pre-populate an item that has received live SSE progress
        var liveItem = new DownloadItem
        {
            Id = "dl-1",
            Filename = "ubuntu.iso",
            Status = DownloadStatus.Downloading,
            BytesDone = 5000,
            LastUpdatedUtc = DateTime.UtcNow.AddSeconds(1) // Fresher than snapshot start
        };
        vm.Downloads.Add(liveItem);

        // Snapshot has older progress
        daemonClient.SnapshotDownloads.Add(new DownloadInfoResponse
        {
            Id = "dl-1",
            Filename = "ubuntu.iso",
            Status = "downloading",
            BytesDone = 2000
        });

        await vm.ReconcileDownloadsAsync();

        // Local item progress must be preserved because it was updated after snapshot began
        Assert.Equal((ulong)5000, liveItem.BytesDone);
    }

    [Fact]
    public async Task ReconcileDownloads_PreservesRecentlyAddedItem_WhenMissingFromStaleSnapshot()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);

        // Item was just added 500ms ago (e.g. via OnAdded)
        var justAdded = new DownloadItem
        {
            Id = "dl-just-added",
            Filename = "movie.mkv",
            Status = DownloadStatus.Connecting,
            CreatedAt = DateTime.UtcNow,
            LastUpdatedUtc = DateTime.UtcNow
        };
        vm.Downloads.Add(justAdded);

        // Snapshot is empty (daemon hasn't committed or query was in-flight)
        daemonClient.SnapshotDownloads.Clear();

        await vm.ReconcileDownloadsAsync();

        // Must NOT remove the item (prevents snapshot merge flicker)
        Assert.Contains(vm.Downloads, d => d.Id == "dl-just-added");
    }

    [Fact]
    public async Task RequestCoalescedReload_CoalescesBurstOfRequestsIntoAtMostTwoCalls()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);

        // Simulate network latency in GetDownloadsAsync
        daemonClient.OnGetDownloadsAsync = async () =>
        {
            await Task.Delay(100);
        };

        // Fire 10 rapid reload requests
        for (int i = 0; i < 10; i++)
        {
            vm.RequestCoalescedReload();
        }

        // Wait for coalesced loop to complete
        await Task.Delay(500);

        // The coalescer should execute at most 2 calls (the active one + 1 follow-up), never 10
        Assert.True(daemonClient.GetDownloadsCallCount <= 2, $"Expected <= 2 calls, got {daemonClient.GetDownloadsCallCount}");
    }

    [Fact]
    public void DownloadItem_PresentsFailureReason_Cleanly()
    {
        var item = new DownloadItem
        {
            Id = "dl-fail",
            Filename = "data.tar.gz",
            Status = DownloadStatus.Failed,
            Error = "HTTP 404 Not Found: remote server returned 404 for resource"
        };

        Assert.True(item.HasError);
        Assert.False(string.IsNullOrEmpty(item.ShortError));
        Assert.True(item.ShortError.Length <= 30);
        Assert.Contains("Failed: HTTP 404", item.ErrorToolTip);

        // When not failed, HasError is false
        item.Status = DownloadStatus.Downloading;
        Assert.False(item.HasError);
    }

    [Fact]
    public void MainWindow_KeyboardRouting_BindsEnterOnlyOnDataGridNotWindow()
    {
        string xamlPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "..", "..", "..", "..", "Vajra.Windows", "Views", "MainWindow.xaml");
        if (!File.Exists(xamlPath))
        {
            // Fallback to relative from repo root if running from bin
            xamlPath = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../windows/Vajra.Windows/Views/MainWindow.xaml"));
        }

        Assert.True(File.Exists(xamlPath), $"Could not find MainWindow.xaml at {xamlPath}");

        string xaml = File.ReadAllText(xamlPath);

        // Verify Window.InputBindings does NOT bind Enter
        int windowInputStart = xaml.IndexOf("<Window.InputBindings>");
        int windowInputEnd = xaml.IndexOf("</Window.InputBindings>");
        Assert.True(windowInputStart > 0 && windowInputEnd > windowInputStart);

        string windowBindings = xaml.Substring(windowInputStart, windowInputEnd - windowInputStart);
        Assert.DoesNotContain("Key=\"Enter\"", windowBindings);

        // Verify DataGrid.InputBindings binds Enter
        int dataGridStart = xaml.IndexOf("<DataGrid");
        Assert.True(dataGridStart > 0);
        string dataGridSection = xaml.Substring(dataGridStart);
        Assert.Contains("<KeyBinding Key=\"Enter\" Command=\"{Binding OpenFileCommand}\"/>", dataGridSection);
    }

    [Fact]
    public async Task ExecuteDelete_WithConfirmDeleteKeepFile_DeletesRecordOnly()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item = new DownloadItem { Id = "dl-keep", Filename = "keep-me.zip", Status = DownloadStatus.Completed };
        vm.Downloads.Add(item);
        vm.SelectedDownload = item;

        vm.ConfirmDeleteCallback = d => DeleteConfirmationResult.KeepFile;

        await ((AsyncRelayCommand)vm.DeleteCommand).ExecuteAsync(null);

        Assert.DoesNotContain(item, vm.Downloads);
        Assert.Equal("dl-keep", daemonClient.LastDeletedId);
        Assert.False(daemonClient.LastDeletedFileParam);
        Assert.Equal(1, daemonClient.DeleteCallCount);
    }

    [Fact]
    public async Task ExecuteDelete_WithConfirmDeleteDeleteFile_DeletesRecordAndDiskFile()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item = new DownloadItem { Id = "dl-del", Filename = "delete-me.iso", Status = DownloadStatus.Completed };
        vm.Downloads.Add(item);
        vm.SelectedDownload = item;

        vm.ConfirmDeleteCallback = d => DeleteConfirmationResult.DeleteFile;

        await ((AsyncRelayCommand)vm.DeleteCommand).ExecuteAsync(null);

        Assert.DoesNotContain(item, vm.Downloads);
        Assert.Equal("dl-del", daemonClient.LastDeletedId);
        Assert.True(daemonClient.LastDeletedFileParam);
        Assert.Equal(1, daemonClient.DeleteCallCount);
    }

    [Fact]
    public async Task ExecuteDelete_WithConfirmDeleteCancel_AbortsDeletion()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item = new DownloadItem { Id = "dl-cancel", Filename = "cancel.mp4", Status = DownloadStatus.Paused };
        vm.Downloads.Add(item);
        vm.SelectedDownload = item;

        vm.ConfirmDeleteCallback = d => DeleteConfirmationResult.Cancel;

        await ((AsyncRelayCommand)vm.DeleteCommand).ExecuteAsync(null);

        Assert.Contains(item, vm.Downloads);
        Assert.Equal(0, daemonClient.DeleteCallCount);
    }

    [Fact]
    public void HasDownloads_ReflectsItemCountState()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        Assert.False(vm.HasDownloads);

        var item = new DownloadItem { Id = "dl-test", Filename = "test.zip" };
        vm.Downloads.Add(item);
        Assert.True(vm.HasDownloads);

        vm.Downloads.Remove(item);
        Assert.False(vm.HasDownloads);
    }

    [Fact]
    public void UpdateSelection_TracksSelectionsAndGeneratesCorrectSummaryText()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-1", Filename = "file1.zip", Status = DownloadStatus.Downloading };
        var item2 = new DownloadItem { Id = "dl-2", Filename = "file2.zip", Status = DownloadStatus.Paused };
        var item3 = new DownloadItem { Id = "dl-3", Filename = "file3.zip", Status = DownloadStatus.Failed };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);
        vm.Downloads.Add(item3);

        // 0 items
        vm.UpdateSelection(new List<DownloadItem>());
        Assert.Equal(0, vm.SelectedCount);
        Assert.False(vm.HasSelection);
        Assert.False(vm.HasMultipleSelection);
        Assert.Equal(string.Empty, vm.SelectionSummaryText);

        // 1 item
        vm.UpdateSelection(new List<DownloadItem> { item1 });
        Assert.Equal(1, vm.SelectedCount);
        Assert.True(vm.HasSelection);
        Assert.False(vm.HasMultipleSelection);
        Assert.Equal("1 item selected", vm.SelectionSummaryText);
        Assert.True(vm.CanPauseSelected);
        Assert.False(vm.CanResumeSelected);
        Assert.False(vm.CanRetrySelected);

        // 2 items
        vm.UpdateSelection(new List<DownloadItem> { item1, item2 });
        Assert.Equal(2, vm.SelectedCount);
        Assert.True(vm.HasSelection);
        Assert.True(vm.HasMultipleSelection);
        Assert.Equal("2 items selected", vm.SelectionSummaryText);
        Assert.True(vm.CanPauseSelected);
        Assert.True(vm.CanResumeSelected);
        Assert.False(vm.CanRetrySelected);

        // 3 items with failed
        vm.UpdateSelection(new List<DownloadItem> { item1, item2, item3 });
        Assert.Equal(3, vm.SelectedCount);
        Assert.Equal("3 items selected", vm.SelectionSummaryText);
        Assert.True(vm.CanRetrySelected);
    }

    [Fact]
    public async Task ExecutePause_WithMultipleSelected_InvokesBulkActionPause()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-1", Filename = "file1.zip", Status = DownloadStatus.Downloading, SpeedBps = 1000 };
        var item2 = new DownloadItem { Id = "dl-2", Filename = "file2.zip", Status = DownloadStatus.Downloading, SpeedBps = 2000 };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);

        vm.UpdateSelection(new List<DownloadItem> { item1, item2 });

        await ((AsyncRelayCommand)vm.PauseCommand).ExecuteAsync(null);

        Assert.NotNull(daemonClient.LastBulkActionRequest);
        Assert.Equal("pause", daemonClient.LastBulkActionRequest!.Action);
        Assert.Equal(new[] { "dl-1", "dl-2" }, daemonClient.LastBulkActionRequest.Ids);
        // Authoritative reconciliation: status does NOT mutate optimistically
        Assert.Equal(DownloadStatus.Downloading, item1.Status);
        Assert.Equal(DownloadStatus.Downloading, item2.Status);

        // When authoritative SSE arrives, UI reflects the update
        sseService.RaiseStateChange(new SseStateChangePayload { Id = "dl-1", Status = "paused" });
        Assert.Equal(DownloadStatus.Paused, item1.Status);
    }

    [Fact]
    public async Task ExecuteResume_WithMultipleSelected_InvokesBulkActionResume()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-1", Filename = "file1.zip", Status = DownloadStatus.Paused };
        var item2 = new DownloadItem { Id = "dl-2", Filename = "file2.zip", Status = DownloadStatus.Paused };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);

        vm.UpdateSelection(new List<DownloadItem> { item1, item2 });

        await ((AsyncRelayCommand)vm.ResumeCommand).ExecuteAsync(null);

        Assert.NotNull(daemonClient.LastBulkActionRequest);
        Assert.Equal("resume", daemonClient.LastBulkActionRequest!.Action);
        Assert.Equal(new[] { "dl-1", "dl-2" }, daemonClient.LastBulkActionRequest.Ids);
        // Authoritative reconciliation: status does NOT mutate optimistically
        Assert.Equal(DownloadStatus.Paused, item1.Status);
        Assert.Equal(DownloadStatus.Paused, item2.Status);

        // When authoritative SSE arrives, UI reflects the update
        sseService.RaiseStateChange(new SseStateChangePayload { Id = "dl-1", Status = "downloading" });
        Assert.Equal(DownloadStatus.Downloading, item1.Status);
    }

    [Fact]
    public async Task ExecuteRetry_SingleAndMultiple_InvokesRetryAndBulkAction()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-err-1", Filename = "file1.zip", Status = DownloadStatus.Failed, Error = "Network drop" };
        var item2 = new DownloadItem { Id = "dl-err-2", Filename = "file2.zip", Status = DownloadStatus.Failed, Error = "Timeout" };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);

        // Single retry
        vm.UpdateSelection(new List<DownloadItem> { item1 });
        await ((AsyncRelayCommand)vm.RetryCommand).ExecuteAsync(null);

        Assert.Equal("dl-err-1", daemonClient.LastRetriedId);
        // Authoritative reconciliation: status remains Failed until SSE signals update
        Assert.Equal(DownloadStatus.Failed, item1.Status);

        // Multi retry
        vm.UpdateSelection(new List<DownloadItem> { item1, item2 });
        await ((AsyncRelayCommand)vm.RetryCommand).ExecuteAsync(null);

        Assert.NotNull(daemonClient.LastBulkActionRequest);
        Assert.Equal("retry", daemonClient.LastBulkActionRequest!.Action);
        Assert.Equal(new[] { "dl-err-1", "dl-err-2" }, daemonClient.LastBulkActionRequest.Ids);
        Assert.Equal(DownloadStatus.Failed, item1.Status);
        Assert.Equal(DownloadStatus.Failed, item2.Status);

        // When authoritative SSE arrives
        sseService.RaiseStateChange(new SseStateChangePayload { Id = "dl-err-1", Status = "connecting" });
        Assert.Equal(DownloadStatus.Connecting, item1.Status);
    }

    [Fact]
    public async Task ExecuteDelete_WithMultipleSelected_InvokesBulkActionDeleteAndRemovesItems()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-del-1", Filename = "file1.zip", Status = DownloadStatus.Completed };
        var item2 = new DownloadItem { Id = "dl-del-2", Filename = "file2.zip", Status = DownloadStatus.Completed };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);

        vm.UpdateSelection(new List<DownloadItem> { item1, item2 });
        vm.ConfirmDeleteMultipleCallback = targets => DeleteConfirmationResult.DeleteFile;

        await ((AsyncRelayCommand)vm.DeleteCommand).ExecuteAsync(null);

        Assert.NotNull(daemonClient.LastBulkActionRequest);
        Assert.Equal("delete", daemonClient.LastBulkActionRequest!.Action);
        Assert.Equal(new[] { "dl-del-1", "dl-del-2" }, daemonClient.LastBulkActionRequest.Ids);
        Assert.True(daemonClient.LastBulkActionRequest.DeleteFile);
        Assert.DoesNotContain(item1, vm.Downloads);
        Assert.DoesNotContain(item2, vm.Downloads);
        Assert.Equal(0, vm.SelectedCount);
    }

    [Fact]
    public async Task ExecuteDelete_PartialFailure_RemovesOnlySucceededItemsAndRetainsFailed()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-1", Filename = "file1.zip", Status = DownloadStatus.Completed };
        var item2 = new DownloadItem { Id = "dl-2", Filename = "file2.zip", Status = DownloadStatus.Downloading };
        var item3 = new DownloadItem { Id = "dl-3", Filename = "file3.zip", Status = DownloadStatus.Failed };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);
        vm.Downloads.Add(item3);

        vm.UpdateSelection(new List<DownloadItem> { item1, item2, item3 });
        vm.ConfirmDeleteMultipleCallback = targets => DeleteConfirmationResult.KeepFile;

        // Mock response where only item1 succeeded, item2 failed due to io_error, item3 failed due to not_found
        // Using reflection or a specialized mock to return partial failure
        daemonClient.BulkActionAsyncCallback = req => Task.FromResult<BulkActionResponse?>(new BulkActionResponse
        {
            Total = 3,
            Succeeded = new List<string> { "dl-1" },
            Failed = new List<BulkActionFailure>
            {
                new BulkActionFailure { Id = "dl-2", Code = "io_error", Message = "File in use by another process" },
                new BulkActionFailure { Id = "dl-3", Code = "not_found", Message = "Download not found" }
            }
        });

        await ((AsyncRelayCommand)vm.DeleteCommand).ExecuteAsync(null);

        // Only dl-1 was confirmed succeeded, so only dl-1 is removed
        Assert.DoesNotContain(item1, vm.Downloads);
        Assert.Contains(item2, vm.Downloads);
        Assert.Contains(item3, vm.Downloads);
        Assert.Equal(2, vm.Downloads.Count);
        Assert.NotNull(vm.ActionStatusMessage);
        Assert.Contains("1 items (2 failed", vm.ActionStatusMessage);
    }

    [Fact]
    public async Task ExecuteDelete_NetworkFailure_MakesNoDestructiveLocalChanges()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-1", Filename = "file1.zip", Status = DownloadStatus.Completed };
        var item2 = new DownloadItem { Id = "dl-2", Filename = "file2.zip", Status = DownloadStatus.Downloading };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);

        vm.UpdateSelection(new List<DownloadItem> { item1, item2 });
        vm.ConfirmDeleteMultipleCallback = targets => DeleteConfirmationResult.KeepFile;

        // Network error -> returns null
        daemonClient.BulkActionAsyncCallback = req => Task.FromResult<BulkActionResponse?>(null);

        await ((AsyncRelayCommand)vm.DeleteCommand).ExecuteAsync(null);

        // Zero destructive local changes!
        Assert.Contains(item1, vm.Downloads);
        Assert.Contains(item2, vm.Downloads);
        Assert.Equal(2, vm.Downloads.Count);
        Assert.Equal("Delete failed: communication error", vm.ActionStatusMessage);
    }

    [Fact]
    public async Task ExecutePauseAll_And_ResumeAll_InvokesDaemonEndpointsWithoutOptimisticMutation()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var item1 = new DownloadItem { Id = "dl-1", Filename = "file1.zip", Status = DownloadStatus.Downloading, SpeedBps = 500 };
        var item2 = new DownloadItem { Id = "dl-2", Filename = "file2.zip", Status = DownloadStatus.Downloading, SpeedBps = 1500 };
        vm.Downloads.Add(item1);
        vm.Downloads.Add(item2);

        await ((AsyncRelayCommand)vm.PauseAllCommand).ExecuteAsync(null);

        // Authoritative: does not alter local status without daemon SSE
        Assert.Equal(DownloadStatus.Downloading, item1.Status);
        Assert.Equal(DownloadStatus.Downloading, item2.Status);

        await ((AsyncRelayCommand)vm.ResumeAllCommand).ExecuteAsync(null);

        Assert.Equal(DownloadStatus.Downloading, item1.Status);
        Assert.Equal(DownloadStatus.Downloading, item2.Status);
    }

    [Fact]
    public async Task ExecuteClearCompleted_RemovesOnlyCompletedDownloads()
    {
        var daemonClient = new MockDaemonClientForSync();
        var sseService = new MockSseServiceForSync();
        var tokenService = new MockTokenService();
        var systemService = new MockSystemInteractionService();

        var vm = new MainViewModel(daemonClient, sseService, tokenService, systemService);
        var itemDone1 = new DownloadItem { Id = "dl-done-1", Filename = "done1.zip", Status = DownloadStatus.Completed };
        var itemActive = new DownloadItem { Id = "dl-active", Filename = "active.zip", Status = DownloadStatus.Downloading };
        var itemDone2 = new DownloadItem { Id = "dl-done-2", Filename = "done2.zip", Status = DownloadStatus.Completed };
        vm.Downloads.Add(itemDone1);
        vm.Downloads.Add(itemActive);
        vm.Downloads.Add(itemDone2);

        daemonClient.OnClearCompletedAsync = () => Task.FromResult<BulkActionResponse?>(new BulkActionResponse
        {
            Total = 2,
            Succeeded = new List<string> { "dl-done-1", "dl-done-2" }
        });

        await ((AsyncRelayCommand)vm.ClearCompletedCommand).ExecuteAsync(null);

        Assert.Single(vm.Downloads);
        Assert.Contains(itemActive, vm.Downloads);
        Assert.DoesNotContain(itemDone1, vm.Downloads);
        Assert.DoesNotContain(itemDone2, vm.Downloads);
    }

    [Fact]
    public void PropertiesViewModel_ExposesDiagnosticDataAccurately()
    {
        var daemonClient = new MockDaemonClientForSync();
        var item = new DownloadItem
        {
            Id = "dl-prop",
            Filename = "ubuntu.iso",
            Url = "https://example.com/ubuntu.iso",
            OutputPath = @"C:\Downloads\ubuntu.iso",
            Status = DownloadStatus.Downloading,
            TotalBytes = 2_000_000_000,
            BytesDone = 1_000_000_000,
            SpeedBps = 10_000_000,
            EtaSeconds = 100,
            ConnectionsActive = 4,
            ResumeSupported = true,
            HashAlgorithm = "sha256",
            ExpectedHash = "abc123expected",
            ActualHash = "abc123expected",
            Segments = new List<SegmentModel>
            {
                new SegmentModel { Id = 0, Start = 0, End = 500_000_000, BytesDone = 500_000_000, Status = "Complete" },
                new SegmentModel { Id = 1, Start = 500_000_001, End = 1_000_000_000, BytesDone = 250_000_000, Status = "Active" }
            }
        };

        var propVm = new PropertiesViewModel(item, daemonClient);

        Assert.Equal("ubuntu.iso", propVm.Filename);
        Assert.Equal("https://example.com/ubuntu.iso", propVm.Url);
        Assert.Equal(@"C:\Downloads\ubuntu.iso", propVm.OutputPath);
        Assert.Equal("Downloading", propVm.StatusText);
        Assert.Equal("4 active", propVm.ActiveConnectionsText);
        Assert.Contains("Yes", propVm.ResumableText);
        Assert.Equal("SHA256", propVm.HashAlgorithmText);
        Assert.Equal("abc123expected", propVm.ExpectedHashText);
        Assert.Equal("abc123expected", propVm.ActualHashText);
        Assert.True(propVm.HasVerifiedHash);
        Assert.False(propVm.HasHashMismatch);
        Assert.Equal(2, propVm.SegmentCount);
        Assert.Equal(2, propVm.Segments.Count);
    }
}
