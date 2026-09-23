using System.IO;
using Vajra.Windows.ViewModels;

namespace Vajra.Windows.Models;

public class DownloadItem : ViewModelBase
{
    private string _id = string.Empty;
    private string _url = string.Empty;
    private string _filename = string.Empty;
    private string? _outputPath;
    private DownloadStatus _status = DownloadStatus.Idle;
    private ulong? _totalBytes;
    private ulong _bytesDone;
    private double _progressPct;
    private ulong _speedBps;
    private ulong? _etaSeconds;
    private DateTime _createdAt = DateTime.Now;
    private string? _error;

    public string Id
    {
        get => _id;
        set => SetProperty(ref _id, value);
    }

    public string Url
    {
        get => _url;
        set => SetProperty(ref _url, value);
    }

    public string Filename
    {
        get => _filename;
        set
        {
            if (SetProperty(ref _filename, value))
            {
                OnPropertyChanged(nameof(Extension));
            }
        }
    }

    public string? OutputPath
    {
        get => _outputPath;
        set
        {
            if (SetProperty(ref _outputPath, value))
            {
                OnPropertyChanged(nameof(CanOpenFile));
                OnPropertyChanged(nameof(CanShowInFolder));
            }
        }
    }

    public DownloadStatus Status
    {
        get => _status;
        set
        {
            if (SetProperty(ref _status, value))
            {
                OnPropertyChanged(nameof(StatusText));
                OnPropertyChanged(nameof(CanPause));
                OnPropertyChanged(nameof(CanResume));
                OnPropertyChanged(nameof(CanRetry));
                OnPropertyChanged(nameof(CanOpenFile));
                OnPropertyChanged(nameof(IsCompleted));
                OnPropertyChanged(nameof(HasError));
                OnPropertyChanged(nameof(ShortError));
                OnPropertyChanged(nameof(FriendlyError));
                OnPropertyChanged(nameof(ErrorToolTip));
            }
        }
    }

    public string StatusText => _status switch
    {
        DownloadStatus.Downloading => "Downloading",
        DownloadStatus.Connecting => "Connecting",
        DownloadStatus.Completed => "Completed",
        DownloadStatus.Paused => "Paused",
        DownloadStatus.Failed => "Failed",
        DownloadStatus.Verifying => "Verifying",
        _ => "Queued"
    };

    public bool HasError => _status == DownloadStatus.Failed && !string.IsNullOrWhiteSpace(_error);

    public string FriendlyError
    {
        get
        {
            if (string.IsNullOrWhiteSpace(_error)) return string.Empty;

            string errLower = _error.ToLowerInvariant();
            if (errLower.Contains("failed to connect") ||
                errLower.Contains("connection refused") ||
                errLower.Contains("actively refused") ||
                errLower.Contains("target machine") ||
                errLower.Contains("connection reset") ||
                errLower.Contains("network unreachable"))
            {
                return "Couldn't connect to server";
            }
            if (errLower.Contains("404") || errLower.Contains("not found"))
            {
                return "File not found (404)";
            }
            if (errLower.Contains("403") || errLower.Contains("forbidden") || errLower.Contains("unauthorized"))
            {
                return "Access denied (403)";
            }
            if (errLower.Contains("500") || errLower.Contains("502") || errLower.Contains("503") || errLower.Contains("504") ||
                errLower.Contains("internal server error") || errLower.Contains("bad gateway"))
            {
                return "Server error";
            }
            if (errLower.Contains("timed out") || errLower.Contains("timeout"))
            {
                return "Connection timed out";
            }
            if (errLower.Contains("disk full") || errLower.Contains("no space") ||
                errLower.Contains("not enough space") || errLower.Contains("out of space") ||
                errLower.Contains("space on disk"))
            {
                return "Disk full";
            }
            if (errLower.Contains("ssl") || errLower.Contains("certificate") || errLower.Contains("tls"))
            {
                return "SSL certificate error";
            }
            if (errLower.Contains("permission denied") || errLower.Contains("access is denied"))
            {
                return "Permission denied";
            }

            string firstLine = _error.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries)[0].Trim();
            return firstLine.Length > 28 ? firstLine.Substring(0, 26) + "..." : firstLine;
        }
    }

    public string ShortError => FriendlyError;

    public string ErrorToolTip
    {
        get
        {
            if (HasError) return $"Failed: {_error}";
            return StatusText;
        }
    }

    public bool IsCompleted => _status == DownloadStatus.Completed;

    public ulong? TotalBytes
    {
        get => _totalBytes;
        set
        {
            if (SetProperty(ref _totalBytes, value))
            {
                OnPropertyChanged(nameof(ProgressText));
                OnPropertyChanged(nameof(TotalBytesFormatted));
            }
        }
    }

    public ulong BytesDone
    {
        get => _bytesDone;
        set
        {
            if (SetProperty(ref _bytesDone, value))
            {
                OnPropertyChanged(nameof(ProgressText));
                OnPropertyChanged(nameof(BytesDoneFormatted));
            }
        }
    }

    public double ProgressPct
    {
        get => _progressPct;
        set => SetProperty(ref _progressPct, value);
    }

    public ulong SpeedBps
    {
        get => _speedBps;
        set
        {
            if (SetProperty(ref _speedBps, value))
            {
                OnPropertyChanged(nameof(SpeedFormatted));
            }
        }
    }

    public ulong? EtaSeconds
    {
        get => _etaSeconds;
        set
        {
            if (SetProperty(ref _etaSeconds, value))
            {
                OnPropertyChanged(nameof(EtaFormatted));
            }
        }
    }

    public DateTime CreatedAt
    {
        get => _createdAt;
        set => SetProperty(ref _createdAt, value);
    }

    public DateTime LastUpdatedUtc { get; set; } = DateTime.UtcNow;

    public string? Error
    {
        get => _error;
        set
        {
            if (SetProperty(ref _error, value))
            {
                OnPropertyChanged(nameof(HasError));
                OnPropertyChanged(nameof(ShortError));
                OnPropertyChanged(nameof(FriendlyError));
                OnPropertyChanged(nameof(ErrorToolTip));
            }
        }
    }

    public string Extension
    {
        get
        {
            if (string.IsNullOrWhiteSpace(Filename)) return "FILE";
            string ext = Path.GetExtension(Filename).TrimStart('.');
            return string.IsNullOrEmpty(ext) ? "FILE" : ext.ToUpperInvariant();
        }
    }

    public string SpeedFormatted
    {
        get
        {
            if (_status != DownloadStatus.Downloading || _speedBps == 0) return "—";
            return $"{FormatBytes(_speedBps)}/s";
        }
    }

    public string EtaFormatted
    {
        get
        {
            if (_status != DownloadStatus.Downloading || !_etaSeconds.HasValue) return "—";
            ulong s = _etaSeconds.Value;
            if (s < 60) return $"{s}s";
            if (s < 3600) return $"{s / 60}m {s % 60}s";
            return $"{s / 3600}h {(s % 3600) / 60}m";
        }
    }

    public string TotalBytesFormatted => FormatBytes(TotalBytes);
    public string BytesDoneFormatted => FormatBytes(BytesDone);

    public string ProgressText
    {
        get
        {
            if (Status == DownloadStatus.Completed)
            {
                return TotalBytes.HasValue ? FormatBytes(TotalBytes) : FormatBytes(BytesDone);
            }

            if (TotalBytes.HasValue && TotalBytes.Value > 0)
            {
                return $"{FormatBytes(BytesDone)} / {FormatBytes(TotalBytes)} ({ProgressPct:0.0}%)";
            }

            return BytesDone > 0 ? $"{FormatBytes(BytesDone)} ({ProgressPct:0.0}%)" : "0%";
        }
    }

    public bool CanPause => _status == DownloadStatus.Downloading || _status == DownloadStatus.Connecting;
    public bool CanResume => _status == DownloadStatus.Paused || _status == DownloadStatus.Failed;
    public bool CanRetry => _status == DownloadStatus.Failed;
    public bool CanOpenFile => _status == DownloadStatus.Completed && !string.IsNullOrWhiteSpace(OutputPath);
    public bool CanShowInFolder => !string.IsNullOrWhiteSpace(OutputPath);

    private byte _connectionsActive;
    private bool _resumeSupported = true;
    private string? _expectedHash;
    private string? _actualHash;
    private string? _hashAlgorithm;
    private long? _completedAtUnix;
    private List<SegmentModel> _segments = new();

    public byte ConnectionsActive
    {
        get => _connectionsActive;
        set => SetProperty(ref _connectionsActive, value);
    }

    public bool ResumeSupported
    {
        get => _resumeSupported;
        set => SetProperty(ref _resumeSupported, value);
    }

    public string? ExpectedHash
    {
        get => _expectedHash;
        set => SetProperty(ref _expectedHash, value);
    }

    public string? ActualHash
    {
        get => _actualHash;
        set
        {
            if (SetProperty(ref _actualHash, value))
            {
                OnPropertyChanged(nameof(HashStatusFormatted));
            }
        }
    }

    public string? HashAlgorithm
    {
        get => _hashAlgorithm;
        set => SetProperty(ref _hashAlgorithm, value);
    }

    public long? CompletedAtUnix
    {
        get => _completedAtUnix;
        set
        {
            if (SetProperty(ref _completedAtUnix, value))
            {
                OnPropertyChanged(nameof(CompletedAtFormatted));
            }
        }
    }

    public List<SegmentModel> Segments
    {
        get => _segments;
        set => SetProperty(ref _segments, value);
    }

    public string CreatedAtFormatted => _createdAt.ToString("yyyy-MM-dd HH:mm:ss");

    public string CompletedAtFormatted => _completedAtUnix.HasValue && _completedAtUnix.Value > 0
        ? DateTimeOffset.FromUnixTimeSeconds(_completedAtUnix.Value).LocalDateTime.ToString("yyyy-MM-dd HH:mm:ss")
        : (_status == DownloadStatus.Completed ? "Completed" : "—");

    public string HashStatusFormatted
    {
        get
        {
            if (string.IsNullOrWhiteSpace(ExpectedHash)) return "None specified";
            if (string.IsNullOrWhiteSpace(ActualHash)) return "Pending verification";
            return string.Equals(ExpectedHash.Trim(), ActualHash.Trim(), StringComparison.OrdinalIgnoreCase)
                ? "Verified Match ✓"
                : "Hash Mismatch ⚠";
        }
    }

    public static string FormatBytes(ulong? bytes)
    {
        if (!bytes.HasValue) return "—";
        double b = bytes.Value;
        string[] units = { "B", "KB", "MB", "GB", "TB" };
        int i = 0;
        while (b >= 1024.0 && i < units.Length - 1)
        {
            b /= 1024.0;
            i++;
        }
        return i == 0 ? $"{b:0} {units[i]}" : $"{b:0.0} {units[i]}";
    }

    public static DownloadStatus ParseStatus(string? statusStr)
    {
        return (statusStr?.ToLowerInvariant()) switch
        {
            "downloading" => DownloadStatus.Downloading,
            "connecting" => DownloadStatus.Connecting,
            "completed" or "complete" => DownloadStatus.Completed,
            "paused" => DownloadStatus.Paused,
            "failed" or "error" => DownloadStatus.Failed,
            "verifying" => DownloadStatus.Verifying,
            _ => DownloadStatus.Idle
        };
    }
}
