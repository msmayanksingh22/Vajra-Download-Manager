using System.Collections.ObjectModel;
using Vajra.Windows.Models;
using Vajra.Windows.Services;

namespace Vajra.Windows.ViewModels;

public class PropertiesViewModel : ViewModelBase
{
    private readonly IDaemonClient _daemonClient;
    private readonly DownloadItem _item;
    private bool _isLoading;

    public DownloadItem Item => _item;

    public ObservableCollection<SegmentModel> Segments { get; } = new();

    public bool IsLoading
    {
        get => _isLoading;
        set => SetProperty(ref _isLoading, value);
    }

    public string Title => $"Properties — {_item.Filename}";

    // General tab
    public string Filename => _item.Filename;
    public string Url => _item.Url;
    public string OutputPath => !string.IsNullOrEmpty(_item.OutputPath) ? _item.OutputPath : "Not set";
    public string StatusText => _item.StatusText;
    public DownloadStatus Status => _item.Status;
    public string FileSizeFormatted => _item.TotalBytesFormatted;
    public string DownloadedFormatted => _item.BytesDoneFormatted;
    public string CreatedAtFormatted => _item.CreatedAtFormatted;
    public string CompletedAtFormatted => _item.CompletedAtFormatted;
    public bool HasError => _item.HasError;
    public string? ErrorMessage => _item.Error;

    // Transfer tab
    public string CurrentSpeedFormatted => _item.SpeedFormatted;
    public string EtaFormatted => _item.EtaFormatted;
    public string ActiveConnectionsText => _item.ConnectionsActive > 0 ? $"{_item.ConnectionsActive} active" : (_item.Status == DownloadStatus.Downloading ? "Connecting..." : "None");
    public string ResumableText => _item.ResumeSupported ? "Yes (HTTP Range supported)" : "No (Single stream only)";
    public string ProgressSummaryText => _item.ProgressText;

    // Integrity tab
    public string HashAlgorithmText => !string.IsNullOrEmpty(_item.HashAlgorithm) ? _item.HashAlgorithm.ToUpperInvariant() : "SHA-256";
    public string ExpectedHashText => !string.IsNullOrEmpty(_item.ExpectedHash) ? _item.ExpectedHash : "None specified";
    public string ActualHashText => !string.IsNullOrEmpty(_item.ActualHash) ? _item.ActualHash : "Pending completion / verification";
    public string HashStatusText => _item.HashStatusFormatted;
    public bool HasVerifiedHash => string.Equals(_item.HashStatusFormatted, "Verified Match ✓", StringComparison.OrdinalIgnoreCase);
    public bool HasHashMismatch => string.Equals(_item.HashStatusFormatted, "Hash Mismatch ⚠", StringComparison.OrdinalIgnoreCase);

    // Segments tab
    public int SegmentCount => Segments.Count;
    public string SegmentsSummaryText => Segments.Count switch
    {
        0 => "No segmented connections (single stream)",
        1 => "1 segment active",
        _ => $"{Segments.Count} segments configured"
    };

    public PropertiesViewModel(DownloadItem item, IDaemonClient daemonClient)
    {
        _item = item;
        _daemonClient = daemonClient;
        UpdateSegments();
    }

    private void UpdateSegments()
    {
        Segments.Clear();
        if (_item.Segments != null)
        {
            foreach (var s in _item.Segments)
            {
                Segments.Add(s);
            }
        }
        OnPropertyChanged(nameof(SegmentCount));
        OnPropertyChanged(nameof(SegmentsSummaryText));
    }

    public async Task RefreshDetailsAsync(CancellationToken ct = default)
    {
        IsLoading = true;
        try
        {
            var details = await _daemonClient.GetDownloadDetailsAsync(_item.Id, ct);
            if (details != null)
            {
                _item.ConnectionsActive = details.ConnectionsActive;
                _item.ResumeSupported = details.ResumeSupported;
                _item.ExpectedHash = details.ExpectedHash;
                _item.ActualHash = details.ActualHash;
                _item.HashAlgorithm = details.HashAlgorithm;
                _item.CompletedAtUnix = details.CompletedAt;
                if (details.Segments != null && details.Segments.Count > 0)
                {
                    _item.Segments = details.Segments;
                }

                UpdateSegments();
                OnPropertyChanged(nameof(StatusText));
                OnPropertyChanged(nameof(Status));
                OnPropertyChanged(nameof(FileSizeFormatted));
                OnPropertyChanged(nameof(DownloadedFormatted));
                OnPropertyChanged(nameof(CompletedAtFormatted));
                OnPropertyChanged(nameof(CurrentSpeedFormatted));
                OnPropertyChanged(nameof(EtaFormatted));
                OnPropertyChanged(nameof(ActiveConnectionsText));
                OnPropertyChanged(nameof(ResumableText));
                OnPropertyChanged(nameof(ProgressSummaryText));
                OnPropertyChanged(nameof(HashAlgorithmText));
                OnPropertyChanged(nameof(ExpectedHashText));
                OnPropertyChanged(nameof(ActualHashText));
                OnPropertyChanged(nameof(HashStatusText));
                OnPropertyChanged(nameof(HasVerifiedHash));
                OnPropertyChanged(nameof(HasHashMismatch));
            }
        }
        catch
        {
            // Fall back to existing cached Item data
        }
        finally
        {
            IsLoading = false;
        }
    }
}
