using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Windows;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Shell;
using Vajra.Windows.Models;
using Vajra.Windows.Services;

namespace Vajra.Windows.ViewModels;

public enum DeleteConfirmationResult
{
    Cancel,
    KeepFile,
    DeleteFile
}

public class MainViewModel : ViewModelBase
{
    private readonly IDaemonClient _daemonClient;
    private readonly ISseService _sseService;
    private readonly ITokenService _tokenService;
    private readonly ISystemInteractionService _systemService;

    private readonly ObservableCollection<DownloadItem> _downloads = new();
    private ICollectionView? _downloadsView;

    private DownloadItem? _selectedDownload;
    private ConnectionState _connectionState = ConnectionState.Connecting;
    private string _searchText = string.Empty;
    private string _statusFilter = "All";
    private string _statusMessage = "Initializing...";
    private ulong _totalSpeedBps;
    private int _activeCount;
    private double _overallProgress;
    private TaskbarItemProgressState _taskbarState = TaskbarItemProgressState.None;
    private System.Threading.Timer? _healthTimer;
    private bool _isCheckingHealth;

    private readonly List<DownloadItem> _selectedDownloads = new();

    public Func<DownloadItem, DeleteConfirmationResult>? ConfirmDeleteCallback { get; set; }
    public Func<IReadOnlyList<DownloadItem>, DeleteConfirmationResult>? ConfirmDeleteMultipleCallback { get; set; }

    public bool HasDownloads => _downloads.Count > 0;

    public ObservableCollection<DownloadItem> Downloads => _downloads;

    public IReadOnlyList<DownloadItem> SelectedDownloads => _selectedDownloads;
    public int SelectedCount => _selectedDownloads.Count;
    public bool HasSelection => _selectedDownloads.Count > 0;
    public bool HasMultipleSelection => _selectedDownloads.Count > 1;

    public string SelectionSummaryText => _selectedDownloads.Count switch
    {
        0 => string.Empty,
        1 => "1 item selected",
        _ => $"{_selectedDownloads.Count} items selected"
    };

    public bool CanPauseSelected => _selectedDownloads.Count > 0 && _selectedDownloads.Any(d => d.CanPause);
    public bool CanResumeSelected => _selectedDownloads.Count > 0 && _selectedDownloads.Any(d => d.CanResume);
    public bool CanRetrySelected => _selectedDownloads.Count > 0 && _selectedDownloads.Any(d => d.CanRetry);
    public bool CanDeleteSelected => _selectedDownloads.Count > 0;

    public void UpdateSelection(System.Collections.IList? selectedItems)
    {
        _selectedDownloads.Clear();
        if (selectedItems != null)
        {
            foreach (var item in selectedItems)
            {
                if (item is DownloadItem download)
                {
                    _selectedDownloads.Add(download);
                }
            }
        }

        _selectedDownload = _selectedDownloads.FirstOrDefault();
        OnPropertyChanged(nameof(SelectedDownload));
        OnPropertyChanged(nameof(SelectedDownloads));
        OnPropertyChanged(nameof(SelectedCount));
        OnPropertyChanged(nameof(HasSelection));
        OnPropertyChanged(nameof(HasMultipleSelection));
        OnPropertyChanged(nameof(SelectionSummaryText));
        OnPropertyChanged(nameof(CanPauseSelected));
        OnPropertyChanged(nameof(CanResumeSelected));
        OnPropertyChanged(nameof(CanRetrySelected));
        OnPropertyChanged(nameof(CanDeleteSelected));

        CommandManager.InvalidateRequerySuggested();
    }

    public ICollectionView DownloadsView
    {
        get
        {
            if (_downloadsView == null)
            {
                _downloadsView = CollectionViewSource.GetDefaultView(_downloads);
                _downloadsView.Filter = FilterDownload;
            }
            return _downloadsView;
        }
    }

    public DownloadItem? SelectedDownload
    {
        get => _selectedDownload;
        set
        {
            if (SetProperty(ref _selectedDownload, value))
            {
                if (value != null && !_selectedDownloads.Contains(value))
                {
                    _selectedDownloads.Clear();
                    _selectedDownloads.Add(value);
                    OnPropertyChanged(nameof(SelectedDownloads));
                    OnPropertyChanged(nameof(SelectedCount));
                    OnPropertyChanged(nameof(HasSelection));
                    OnPropertyChanged(nameof(HasMultipleSelection));
                    OnPropertyChanged(nameof(SelectionSummaryText));
                    OnPropertyChanged(nameof(CanPauseSelected));
                    OnPropertyChanged(nameof(CanResumeSelected));
                    OnPropertyChanged(nameof(CanRetrySelected));
                    OnPropertyChanged(nameof(CanDeleteSelected));
                }
                else if (value == null && _selectedDownloads.Count > 0)
                {
                    _selectedDownloads.Clear();
                    OnPropertyChanged(nameof(SelectedDownloads));
                    OnPropertyChanged(nameof(SelectedCount));
                    OnPropertyChanged(nameof(HasSelection));
                    OnPropertyChanged(nameof(HasMultipleSelection));
                    OnPropertyChanged(nameof(SelectionSummaryText));
                    OnPropertyChanged(nameof(CanPauseSelected));
                    OnPropertyChanged(nameof(CanResumeSelected));
                    OnPropertyChanged(nameof(CanRetrySelected));
                    OnPropertyChanged(nameof(CanDeleteSelected));
                }
                CommandManager.InvalidateRequerySuggested();
            }
        }
    }

    public ConnectionState ConnectionState
    {
        get => _connectionState;
        set
        {
            if (SetProperty(ref _connectionState, value))
            {
                OnPropertyChanged(nameof(ConnectionText));
                OnPropertyChanged(nameof(IsConnected));
            }
        }
    }

    public bool IsConnected => _connectionState == ConnectionState.Connected;

    public string ConnectionText => _connectionState switch
    {
        ConnectionState.Connected => $"Connected (127.0.0.1:{_tokenService.GetPort()})",
        ConnectionState.Reconnecting => "Reconnecting to Vajra daemon...",
        ConnectionState.Connecting => "Connecting...",
        ConnectionState.Unauthorized => "Authentication Failed — Invalid API Token",
        _ => $"Daemon Unavailable (127.0.0.1:{_tokenService.GetPort()})"
    };

    public string SearchText
    {
        get => _searchText;
        set
        {
            if (SetProperty(ref _searchText, value))
            {
                DownloadsView.Refresh();
            }
        }
    }

    public string StatusFilter
    {
        get => _statusFilter;
        set
        {
            if (SetProperty(ref _statusFilter, value))
            {
                DownloadsView.Refresh();
            }
        }
    }

    public string StatusMessage
    {
        get => _statusMessage;
        set => SetProperty(ref _statusMessage, value);
    }

    public ulong TotalSpeedBps
    {
        get => _totalSpeedBps;
        set
        {
            if (SetProperty(ref _totalSpeedBps, value))
            {
                OnPropertyChanged(nameof(TotalSpeedFormatted));
            }
        }
    }

    public string TotalSpeedFormatted
    {
        get
        {
            if (_totalSpeedBps == 0 || !IsConnected) return "—";
            return $"{DownloadItem.FormatBytes(_totalSpeedBps)}/s";
        }
    }

    public int ActiveCount
    {
        get => _activeCount;
        set => SetProperty(ref _activeCount, value);
    }

    public string ItemCountText => $"{_downloads.Count} {(_downloads.Count == 1 ? "download" : "downloads")}";

    public double OverallProgress
    {
        get => _overallProgress;
        set => SetProperty(ref _overallProgress, value);
    }

    public TaskbarItemProgressState TaskbarState
    {
        get => _taskbarState;
        set => SetProperty(ref _taskbarState, value);
    }

    // Commands
    public ICommand AddUrlCommand { get; }
    public ICommand PauseCommand { get; }
    public ICommand ResumeCommand { get; }
    public ICommand RetryCommand { get; }
    public ICommand DeleteCommand { get; }
    public ICommand PauseAllCommand { get; }
    public ICommand ResumeAllCommand { get; }
    public ICommand ClearCompletedCommand { get; }
    public ICommand OpenFileCommand { get; }
    public ICommand ShowInFolderCommand { get; }
    public ICommand CopyUrlCommand { get; }
    public ICommand RefreshCommand { get; }
    public ICommand OpenPropertiesCommand { get; }

    public event Action? RequestAddUrlDialog;
    public event Action<DownloadItem>? RequestPropertiesDialog;

    public MainViewModel(
        IDaemonClient daemonClient,
        ISseService sseService,
        ITokenService tokenService,
        ISystemInteractionService systemService)
    {
        _daemonClient = daemonClient;
        _sseService = sseService;
        _tokenService = tokenService;
        _systemService = systemService;

        AddUrlCommand = new RelayCommand(() => RequestAddUrlDialog?.Invoke(), () => IsConnected);
        PauseCommand = new AsyncRelayCommand(ExecutePauseAsync, () => IsConnected && (SelectedDownload?.CanPause == true || CanPauseSelected));
        ResumeCommand = new AsyncRelayCommand(ExecuteResumeAsync, () => IsConnected && (SelectedDownload?.CanResume == true || CanResumeSelected));
        RetryCommand = new AsyncRelayCommand(ExecuteRetryAsync, () => IsConnected && (SelectedDownload?.CanRetry == true || CanRetrySelected));
        DeleteCommand = new AsyncRelayCommand(ExecuteDeleteAsync, () => IsConnected && (SelectedDownload != null || CanDeleteSelected));
        PauseAllCommand = new AsyncRelayCommand(ExecutePauseAllAsync, () => IsConnected && _downloads.Any(d => d.CanPause));
        ResumeAllCommand = new AsyncRelayCommand(ExecuteResumeAllAsync, () => IsConnected && _downloads.Any(d => d.CanResume));
        ClearCompletedCommand = new AsyncRelayCommand(ExecuteClearCompletedAsync, () => IsConnected && _downloads.Any(d => d.IsCompleted));
        OpenFileCommand = new RelayCommand(ExecuteOpenFile, () => SelectedDownload?.CanOpenFile == true);
        ShowInFolderCommand = new RelayCommand(ExecuteShowInFolder, () => SelectedDownload?.CanShowInFolder == true);
        CopyUrlCommand = new RelayCommand(ExecuteCopyUrl, () => !string.IsNullOrEmpty(SelectedDownload?.Url));
        RefreshCommand = new AsyncRelayCommand(ReconcileDownloadsAsync, () => IsConnected);
        OpenPropertiesCommand = new RelayCommand(ExecuteOpenProperties, () => SelectedDownload != null);

        SubscribeToSse();
    }

    private int _reloadRequested;
    private readonly object _reloadLock = new();

    public void RequestCoalescedReload()
    {
        lock (_reloadLock)
        {
            if (_reloadRequested > 0)
            {
                _reloadRequested = 2; // Queue follow-up reload
                return;
            }
            _reloadRequested = 1;
        }

        _ = Task.Run(async () =>
        {
            while (true)
            {
                await ReconcileDownloadsAsync();

                lock (_reloadLock)
                {
                    if (_reloadRequested == 2)
                    {
                        _reloadRequested = 1;
                    }
                    else
                    {
                        _reloadRequested = 0;
                        break;
                    }
                }
            }
        });
    }

    public void Initialize()
    {
        // 1. Establish SSE stream
        _ = _sseService.StartAsync();

        // 2. Perform initial snapshot fetch & reconciliation
        RequestCoalescedReload();

        // 3. Start background health heartbeat
        _healthTimer = new System.Threading.Timer(OnHealthTimerTick, null, 3000, 3000);
    }

    public void Cleanup()
    {
        _healthTimer?.Dispose();
        _healthTimer = null;
        _ = _sseService.StopAsync();
    }

    private void SubscribeToSse()
    {
        _sseService.OnConnected += () =>
        {
            RunOnUi(() =>
            {
                ConnectionState = ConnectionState.Connected;
                StatusMessage = "Ready";
            });
            // Immediate post-connect reconciliation snapshot to catch any events missed during connection handshake
            RequestCoalescedReload();
        };

        _sseService.OnDisconnected += () =>
        {
            RunOnUi(() =>
            {
                if (ConnectionState == ConnectionState.Connected)
                {
                    ConnectionState = ConnectionState.Reconnecting;
                    StatusMessage = "Connection interrupted, reconnecting...";
                }
            });
        };

        _sseService.OnAuthenticationFailed += () =>
        {
            RunOnUi(() =>
            {
                ConnectionState = ConnectionState.Unauthorized;
                StatusMessage = "Authentication failed: invalid or unauthorized API token";
            });
        };

        _sseService.OnProgress += (payload) =>
        {
            RunOnUi(() => ApplyProgress(payload));
        };

        _sseService.OnBatchProgress += (batch) =>
        {
            RunOnUi(() =>
            {
                foreach (var p in batch.Downloads)
                {
                    ApplyProgress(p);
                }
                RecalculateTotals();
            });
        };

        _sseService.OnStateChange += (payload) =>
        {
            RunOnUi(() =>
            {
                var item = FindItem(payload.Id);
                if (item != null)
                {
                    item.LastUpdatedUtc = DateTime.UtcNow;
                    item.Status = DownloadItem.ParseStatus(payload.Status);
                    if (!string.IsNullOrEmpty(payload.OutputPath))
                    {
                        item.OutputPath = payload.OutputPath;
                    }
                    if (!string.IsNullOrEmpty(payload.Error))
                    {
                        item.Error = payload.Error;
                    }
                    RecalculateTotals();
                }
            });
        };

        _sseService.OnAdded += (payload) =>
        {
            RunOnUi(() =>
            {
                var existing = FindItem(payload.Id);
                if (existing == null)
                {
                    var newItem = new DownloadItem
                    {
                        Id = payload.Id,
                        Url = payload.Url,
                        Filename = payload.Filename,
                        Status = DownloadStatus.Connecting,
                        CreatedAt = DateTime.UtcNow,
                        LastUpdatedUtc = DateTime.UtcNow
                    };
                    _downloads.Insert(0, newItem);
                    OnPropertyChanged(nameof(ItemCountText));
                    OnPropertyChanged(nameof(HasDownloads));
                }
                // Coalesced reload ensures metadata and IDs stay in sync without multiple concurrent reloads
                RequestCoalescedReload();
            });
        };

        _sseService.OnRemoved += (payload) =>
        {
            RunOnUi(() =>
            {
                var item = FindItem(payload.Id);
                if (item != null)
                {
                    _downloads.Remove(item);
                    OnPropertyChanged(nameof(ItemCountText));
                    OnPropertyChanged(nameof(HasDownloads));
                    RecalculateTotals();
                }
            });
        };
    }

    private void ApplyProgress(SseProgressPayload payload)
    {
        var item = FindItem(payload.DownloadId);
        if (item != null)
        {
            item.LastUpdatedUtc = DateTime.UtcNow;
            item.BytesDone = payload.DownloadedBytes;
            if (payload.TotalBytes.HasValue && payload.TotalBytes.Value > 0)
            {
                item.TotalBytes = payload.TotalBytes;
                item.ProgressPct = ((double)payload.DownloadedBytes / payload.TotalBytes.Value) * 100.0;
            }
            item.SpeedBps = payload.SpeedBps;
            item.EtaSeconds = payload.EtaSeconds;
            item.Status = DownloadItem.ParseStatus(payload.Status);
            if (!string.IsNullOrEmpty(payload.Filename))
            {
                item.Filename = payload.Filename;
            }
            if (!string.IsNullOrEmpty(payload.Error))
            {
                item.Error = payload.Error;
            }
            if (payload.Segments != null && payload.Segments.Count > 0)
            {
                item.Segments = payload.Segments;
            }
        }
        else
        {
            // Coalesce reload requests when an unknown ID arrives
            RequestCoalescedReload();
        }

        RecalculateTotals();
    }

    private void RecalculateTotals()
    {
        ulong totalSpeed = 0;
        int active = 0;
        ulong totalDownloaded = 0;
        ulong totalExpected = 0;

        foreach (var item in _downloads)
        {
            if (item.Status == DownloadStatus.Downloading)
            {
                totalSpeed += item.SpeedBps;
                active++;
            }

            if (item.Status == DownloadStatus.Downloading || item.Status == DownloadStatus.Connecting)
            {
                totalDownloaded += item.BytesDone;
                if (item.TotalBytes.HasValue)
                {
                    totalExpected += item.TotalBytes.Value;
                }
            }
        }

        TotalSpeedBps = totalSpeed;
        ActiveCount = active;

        // Windows Taskbar progress integration
        if (active > 0)
        {
            TaskbarState = TaskbarItemProgressState.Normal;
            OverallProgress = totalExpected > 0 ? Math.Clamp((double)totalDownloaded / totalExpected, 0.0, 1.0) : 0.0;
        }
        else
        {
            TaskbarState = TaskbarItemProgressState.None;
            OverallProgress = 0.0;
        }
    }

    private DownloadItem? FindItem(string id)
    {
        return _downloads.FirstOrDefault(d => string.Equals(d.Id, id, StringComparison.OrdinalIgnoreCase));
    }

    private async void OnHealthTimerTick(object? state)
    {
        if (_isCheckingHealth) return;
        _isCheckingHealth = true;

        try
        {
            var health = await _daemonClient.CheckHealthAsync();
            bool wasDisconnected = ConnectionState != ConnectionState.Connected;

            if (health != null && health.Status == "ok")
            {
                if (ConnectionState == ConnectionState.Unauthorized)
                {
                    // Do not override Unauthorized status from unauthenticated health check
                    return;
                }

                if (wasDisconnected)
                {
                    RunOnUi(() =>
                    {
                        ConnectionState = ConnectionState.Connected;
                        StatusMessage = "Connected to Vajra daemon";
                    });
                    _tokenService.InvalidateCache();
                    RequestCoalescedReload();
                }
            }
            else
            {
                if (ConnectionState != ConnectionState.Unauthorized)
                {
                    RunOnUi(() =>
                    {
                        ConnectionState = ConnectionState.Disconnected;
                        StatusMessage = "Vajra daemon offline";
                        TotalSpeedBps = 0;
                        ActiveCount = 0;
                    });
                }
            }
        }
        catch
        {
            if (ConnectionState != ConnectionState.Unauthorized)
            {
                RunOnUi(() =>
                {
                    ConnectionState = ConnectionState.Disconnected;
                    StatusMessage = "Vajra daemon unreachable";
                });
            }
        }
        finally
        {
            _isCheckingHealth = false;
        }
    }

    public Task LoadDownloadsAsync() => ReconcileDownloadsAsync();

    public async Task ReconcileDownloadsAsync()
    {
        try
        {
            var snapshotStartTime = DateTime.UtcNow;
            var remoteList = await _daemonClient.GetDownloadsAsync();
            RunOnUi(() =>
            {
                var remoteMap = remoteList.ToDictionary(i => i.Id, StringComparer.OrdinalIgnoreCase);

                // Update existing or remove missing
                for (int i = _downloads.Count - 1; i >= 0; i--)
                {
                    var local = _downloads[i];
                    if (remoteMap.TryGetValue(local.Id, out var remote))
                    {
                        // If local item was updated by live SSE after snapshot query began, do not overwrite with stale snapshot progress
                        if (local.LastUpdatedUtc <= snapshotStartTime)
                        {
                            UpdateItemFromDto(local, remote);
                        }
                        else
                        {
                            // Local has newer SSE data; backfill static path if missing
                            if (string.IsNullOrEmpty(local.OutputPath) && !string.IsNullOrEmpty(remote.OutputPath))
                            {
                                local.OutputPath = remote.OutputPath;
                            }
                        }
                        remoteMap.Remove(local.Id);
                    }
                    else
                    {
                        // Item exists locally but is absent from snapshot.
                        // Prevent snapshot merge flicker: if item was added recently (within 5 seconds)
                        // or updated after snapshot request was initiated, keep it.
                        bool isRecentlyAdded = (DateTime.UtcNow - local.CreatedAt).TotalSeconds < 5.0
                                               || local.LastUpdatedUtc > snapshotStartTime;
                        if (!isRecentlyAdded)
                        {
                            _downloads.RemoveAt(i);
                        }
                    }
                }

                // Add newly discovered remote items
                foreach (var missing in remoteMap.Values)
                {
                    var newItem = new DownloadItem();
                    UpdateItemFromDto(newItem, missing);
                    _downloads.Add(newItem);
                }

                OnPropertyChanged(nameof(ItemCountText));
                OnPropertyChanged(nameof(HasDownloads));
                RecalculateTotals();
            });
        }
        catch
        {
            // Network or parse error
        }
    }

    private static void UpdateItemFromDto(DownloadItem item, DownloadInfoResponse dto)
    {
        item.Id = dto.Id;
        item.Url = dto.Url;
        item.Filename = dto.Filename;
        item.OutputPath = dto.OutputPath;
        item.Status = DownloadItem.ParseStatus(dto.Status);
        item.TotalBytes = dto.TotalBytes;
        item.BytesDone = dto.BytesDone;
        item.SpeedBps = dto.SpeedBps;
        item.EtaSeconds = dto.EtaSeconds;
        item.ProgressPct = dto.ProgressPct;
        item.CreatedAt = dto.CreatedAt > 0 ? DateTimeOffset.FromUnixTimeSeconds(dto.CreatedAt).LocalDateTime : DateTime.Now;
        item.Error = dto.Error;
        item.ConnectionsActive = dto.ConnectionsActive;
        item.ResumeSupported = dto.ResumeSupported;
        item.ExpectedHash = dto.ExpectedHash;
        item.ActualHash = dto.ActualHash;
        item.HashAlgorithm = dto.HashAlgorithm;
        item.CompletedAtUnix = dto.CompletedAt;
        if (dto.Segments != null && dto.Segments.Count > 0)
        {
            item.Segments = dto.Segments;
        }
    }

    private bool FilterDownload(object item)
    {
        if (item is not DownloadItem download) return false;

        // Search text
        if (!string.IsNullOrWhiteSpace(_searchText))
        {
            bool matchName = download.Filename.Contains(_searchText, StringComparison.OrdinalIgnoreCase);
            bool matchUrl = download.Url.Contains(_searchText, StringComparison.OrdinalIgnoreCase);
            if (!matchName && !matchUrl) return false;
        }

        // Status filter
        if (_statusFilter == "Active")
        {
            return download.Status == DownloadStatus.Downloading || download.Status == DownloadStatus.Connecting;
        }
        if (_statusFilter == "Completed")
        {
            return download.Status == DownloadStatus.Completed;
        }
        if (_statusFilter == "Paused")
        {
            return download.Status == DownloadStatus.Paused;
        }

        return true;
    }

    private string? _actionStatusMessage;
    public string? ActionStatusMessage
    {
        get => _actionStatusMessage;
        set => SetProperty(ref _actionStatusMessage, value);
    }

    private void HandleBulkResponse(BulkActionResponse? resp, string actionName)
    {
        if (resp == null)
        {
            ActionStatusMessage = $"{actionName} failed: communication error";
            return;
        }

        if (resp.Failed.Count > 0)
        {
            var first = resp.Failed[0];
            string reason = !string.IsNullOrEmpty(first.Message) ? first.Message : first.Code;
            ActionStatusMessage = $"{actionName}: {resp.Succeeded.Count} succeeded, {resp.Failed.Count} failed ({reason})";
        }
        else
        {
            ActionStatusMessage = $"{actionName}: {resp.Succeeded.Count} succeeded";
        }
    }

    private void ReconcileSelectionProperties()
    {
        OnPropertyChanged(nameof(ItemCountText));
        OnPropertyChanged(nameof(HasDownloads));
        OnPropertyChanged(nameof(SelectedCount));
        OnPropertyChanged(nameof(HasSelection));
        OnPropertyChanged(nameof(HasMultipleSelection));
        OnPropertyChanged(nameof(SelectionSummaryText));
    }

    private async Task ExecutePauseAsync()
    {
        if (_selectedDownloads.Count > 1)
        {
            var targets = _selectedDownloads.Where(d => d.CanPause).ToList();
            if (targets.Count == 0) return;
            var ids = targets.Select(d => d.Id).ToList();
            var resp = await _daemonClient.BulkActionAsync(new BulkActionRequest
            {
                Action = "pause",
                Ids = ids
            });
            HandleBulkResponse(resp, "Pause");
        }
        else if (SelectedDownload != null && SelectedDownload.CanPause)
        {
            string id = SelectedDownload.Id;
            bool ok = await _daemonClient.PauseDownloadAsync(id);
            if (!ok)
            {
                ActionStatusMessage = "Failed to pause download";
            }
        }
    }

    private async Task ExecuteResumeAsync()
    {
        if (_selectedDownloads.Count > 1)
        {
            var targets = _selectedDownloads.Where(d => d.CanResume).ToList();
            if (targets.Count == 0) return;
            var ids = targets.Select(d => d.Id).ToList();
            var resp = await _daemonClient.BulkActionAsync(new BulkActionRequest
            {
                Action = "resume",
                Ids = ids
            });
            HandleBulkResponse(resp, "Resume");
        }
        else if (SelectedDownload != null && SelectedDownload.CanResume)
        {
            string id = SelectedDownload.Id;
            bool ok = await _daemonClient.ResumeDownloadAsync(id);
            if (!ok)
            {
                ActionStatusMessage = "Failed to resume download";
            }
        }
    }

    private async Task ExecuteRetryAsync()
    {
        if (_selectedDownloads.Count > 1)
        {
            var targets = _selectedDownloads.Where(d => d.CanRetry).ToList();
            if (targets.Count == 0) return;
            var ids = targets.Select(d => d.Id).ToList();
            var resp = await _daemonClient.BulkActionAsync(new BulkActionRequest
            {
                Action = "retry",
                Ids = ids
            });
            HandleBulkResponse(resp, "Retry");
        }
        else if (SelectedDownload != null && SelectedDownload.CanRetry)
        {
            string id = SelectedDownload.Id;
            bool ok = await _daemonClient.RetryDownloadAsync(id);
            if (!ok)
            {
                ActionStatusMessage = "Failed to retry download";
            }
        }
    }

    private async Task ExecuteDeleteAsync()
    {
        if (_selectedDownloads.Count > 1)
        {
            var targets = _selectedDownloads.ToList();
            DeleteConfirmationResult result;
            if (ConfirmDeleteMultipleCallback != null)
            {
                result = ConfirmDeleteMultipleCallback(targets);
            }
            else
            {
                var msgResult = MessageBox.Show(
                    $"Are you sure you want to remove {targets.Count} downloads from the download list?\n\nClick 'Yes' to remove from list and delete downloaded files from disk.\nClick 'No' to remove from list only.\nClick 'Cancel' to keep the downloads.",
                    "Confirm Delete",
                    MessageBoxButton.YesNoCancel,
                    MessageBoxImage.Question);

                result = msgResult switch
                {
                    MessageBoxResult.Yes => DeleteConfirmationResult.DeleteFile,
                    MessageBoxResult.No => DeleteConfirmationResult.KeepFile,
                    _ => DeleteConfirmationResult.Cancel
                };
            }

            if (result == DeleteConfirmationResult.Cancel) return;

            bool deleteFiles = result == DeleteConfirmationResult.DeleteFile;
            var ids = targets.Select(d => d.Id).ToList();

            var resp = await _daemonClient.BulkActionAsync(new BulkActionRequest
            {
                Action = "delete",
                Ids = ids,
                DeleteFile = deleteFiles
            });

            if (resp != null)
            {
                var succeededSet = new HashSet<string>(resp.Succeeded);
                for (int i = _downloads.Count - 1; i >= 0; i--)
                {
                    if (succeededSet.Contains(_downloads[i].Id))
                    {
                        _downloads.RemoveAt(i);
                    }
                }
                _selectedDownloads.RemoveAll(d => succeededSet.Contains(d.Id));
                if (SelectedDownload != null && succeededSet.Contains(SelectedDownload.Id))
                {
                    SelectedDownload = _selectedDownloads.FirstOrDefault();
                }
                ReconcileSelectionProperties();
                RecalculateTotals();

                if (resp.Failed.Count > 0)
                {
                    var first = resp.Failed[0];
                    string reason = !string.IsNullOrEmpty(first.Message) ? first.Message : first.Code;
                    ActionStatusMessage = $"Deleted {resp.Succeeded.Count} items ({resp.Failed.Count} failed: {reason})";
                }
                else
                {
                    ActionStatusMessage = $"Deleted {resp.Succeeded.Count} items";
                }
            }
            else
            {
                ActionStatusMessage = "Delete failed: communication error";
            }
        }
        else if (SelectedDownload != null)
        {
            var item = SelectedDownload;
            DeleteConfirmationResult result;
            if (ConfirmDeleteCallback != null)
            {
                result = ConfirmDeleteCallback(item);
            }
            else
            {
                var msgResult = MessageBox.Show(
                    $"Are you sure you want to remove '{item.Filename}' from the download list?\n\nClick 'Yes' to remove from list and delete downloaded file from disk.\nClick 'No' to remove from list only.\nClick 'Cancel' to keep the download.",
                    "Confirm Delete",
                    MessageBoxButton.YesNoCancel,
                    MessageBoxImage.Question);

                result = msgResult switch
                {
                    MessageBoxResult.Yes => DeleteConfirmationResult.DeleteFile,
                    MessageBoxResult.No => DeleteConfirmationResult.KeepFile,
                    _ => DeleteConfirmationResult.Cancel
                };
            }

            if (result == DeleteConfirmationResult.Cancel) return;

            bool deleteFile = result == DeleteConfirmationResult.DeleteFile;
            bool ok = await _daemonClient.DeleteDownloadAsync(item.Id, deleteFile);
            if (ok)
            {
                _downloads.Remove(item);
                _selectedDownloads.Remove(item);
                if (SelectedDownload == item)
                {
                    SelectedDownload = _selectedDownloads.FirstOrDefault();
                }
                ReconcileSelectionProperties();
                RecalculateTotals();
                ActionStatusMessage = $"Deleted '{item.Filename}'";
            }
            else
            {
                ActionStatusMessage = $"Failed to delete '{item.Filename}'";
            }
        }
    }

    private async Task ExecutePauseAllAsync()
    {
        bool ok = await _daemonClient.PauseAllAsync();
        if (!ok)
        {
            ActionStatusMessage = "Failed to pause all downloads";
        }
    }

    private async Task ExecuteResumeAllAsync()
    {
        bool ok = await _daemonClient.ResumeAllAsync();
        if (!ok)
        {
            ActionStatusMessage = "Failed to resume all downloads";
        }
    }

    private async Task ExecuteClearCompletedAsync()
    {
        var resp = await _daemonClient.ClearCompletedAsync();
        if (resp != null)
        {
            var succeededSet = new HashSet<string>(resp.Succeeded);
            for (int i = _downloads.Count - 1; i >= 0; i--)
            {
                if (succeededSet.Contains(_downloads[i].Id))
                {
                    _downloads.RemoveAt(i);
                }
            }
            _selectedDownloads.RemoveAll(d => succeededSet.Contains(d.Id));
            if (SelectedDownload != null && succeededSet.Contains(SelectedDownload.Id))
            {
                SelectedDownload = _selectedDownloads.FirstOrDefault();
            }
            ReconcileSelectionProperties();
            RecalculateTotals();

            if (resp.Failed.Count > 0)
            {
                var first = resp.Failed[0];
                string reason = !string.IsNullOrEmpty(first.Message) ? first.Message : first.Code;
                ActionStatusMessage = $"Cleared {resp.Succeeded.Count} completed downloads ({resp.Failed.Count} failed: {reason})";
            }
            else
            {
                ActionStatusMessage = $"Cleared {resp.Succeeded.Count} completed downloads";
            }
        }
        else
        {
            ActionStatusMessage = "Clear completed failed: communication error";
        }
    }

    private void ExecuteOpenProperties()
    {
        if (SelectedDownload != null)
        {
            RequestPropertiesDialog?.Invoke(SelectedDownload);
        }
    }

    private void ExecuteOpenFile()
    {
        if (SelectedDownload?.OutputPath != null)
        {
            if (!_systemService.OpenFile(SelectedDownload.OutputPath))
            {
                MessageBox.Show($"Could not open file: '{SelectedDownload.OutputPath}'.\nThe file may have been moved, renamed, or deleted.", "Open File", MessageBoxButton.OK, MessageBoxImage.Warning);
            }
        }
    }

    private void ExecuteShowInFolder()
    {
        if (SelectedDownload?.OutputPath != null)
        {
            if (!_systemService.ShowInFolder(SelectedDownload.OutputPath))
            {
                MessageBox.Show($"Could not locate directory: '{SelectedDownload.OutputPath}'.", "Show in Folder", MessageBoxButton.OK, MessageBoxImage.Warning);
            }
        }
    }

    private void ExecuteCopyUrl()
    {
        if (!string.IsNullOrEmpty(SelectedDownload?.Url))
        {
            _systemService.CopyToClipboard(SelectedDownload.Url);
            StatusMessage = "Download URL copied to clipboard";
        }
    }

    private static void RunOnUi(Action action)
    {
        if (Application.Current?.Dispatcher != null)
        {
            if (Application.Current.Dispatcher.CheckAccess())
            {
                action();
            }
            else
            {
                Application.Current.Dispatcher.InvokeAsync(action);
            }
        }
        else
        {
            action();
        }
    }
}
