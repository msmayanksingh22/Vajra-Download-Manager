using Vajra.Windows.Models;

namespace Vajra.Windows.Services;

public interface IDaemonClient
{
    Task<HealthResponse?> CheckHealthAsync(CancellationToken ct = default);
    Task<List<DownloadInfoResponse>> GetDownloadsAsync(CancellationToken ct = default);
    Task<AddDownloadResponse?> AddDownloadAsync(string url, string? filename = null, string? outputDir = null, CancellationToken ct = default);
    Task<bool> PauseDownloadAsync(string id, CancellationToken ct = default);
    Task<bool> ResumeDownloadAsync(string id, CancellationToken ct = default);
    Task<bool> DeleteDownloadAsync(string id, bool deleteFile = false, CancellationToken ct = default);
    Task<BulkActionResponse?> BulkActionAsync(BulkActionRequest request, CancellationToken ct = default);
    Task<bool> PauseAllAsync(CancellationToken ct = default);
    Task<bool> ResumeAllAsync(CancellationToken ct = default);
    Task<BulkActionResponse?> ClearCompletedAsync(CancellationToken ct = default);
    Task<bool> RetryDownloadAsync(string id, CancellationToken ct = default);
    Task<DownloadInfoResponse?> GetDownloadDetailsAsync(string id, CancellationToken ct = default);
}
