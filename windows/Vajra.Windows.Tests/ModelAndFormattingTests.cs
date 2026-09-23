using System.Text.Json;
using Vajra.Windows.Models;
using Xunit;

namespace Vajra.Windows.Tests;

public class ModelAndFormattingTests
{
    [Fact]
    public void FormatBytes_FormatsAccurately()
    {
        Assert.Equal("—", DownloadItem.FormatBytes(null));
        Assert.Equal("500 B", DownloadItem.FormatBytes(500));
        Assert.Equal("1.0 KB", DownloadItem.FormatBytes(1024));
        Assert.Equal("1.5 MB", DownloadItem.FormatBytes((ulong)(1.5 * 1024 * 1024)));
        Assert.Equal("4.2 GB", DownloadItem.FormatBytes((ulong)(4.2 * 1024 * 1024 * 1024)));
    }

    [Theory]
    [InlineData("archive.zip", "ZIP")]
    [InlineData("video.mp4", "MP4")]
    [InlineData("setup.exe", "EXE")]
    [InlineData("ubuntu-24.04.iso", "ISO")]
    [InlineData("LICENSE", "FILE")]
    [InlineData("", "FILE")]
    [InlineData(null, "FILE")]
    public void DownloadItem_ExtractsExtensionCorrectly(string? filename, string expectedExt)
    {
        var item = new DownloadItem { Filename = filename ?? "" };
        Assert.Equal(expectedExt, item.Extension);
    }

    [Fact]
    public void DownloadItem_ComputesSpeedAndEta()
    {
        var item = new DownloadItem
        {
            Status = DownloadStatus.Downloading,
            SpeedBps = 10 * 1024 * 1024, // 10 MB/s
            EtaSeconds = 65 // 1m 5s
        };

        Assert.Equal("10.0 MB/s", item.SpeedFormatted);
        Assert.Equal("1m 5s", item.EtaFormatted);

        // When paused, speed and eta should be formatted as "—"
        item.Status = DownloadStatus.Paused;
        Assert.Equal("—", item.SpeedFormatted);
        Assert.Equal("—", item.EtaFormatted);
    }

    [Fact]
    public void DownloadInfoResponse_DeserializesProperly()
    {
        string json = """
        {
            "total": 1,
            "limit": 50,
            "offset": 0,
            "items": [
                {
                    "id": "11111111-2222-3333-4444-555555555555",
                    "status": "downloading",
                    "url": "https://example.com/test.iso",
                    "output_path": "C:\\Downloads\\test.iso",
                    "filename": "test.iso",
                    "total_bytes": 1073741824,
                    "bytes_done": 536870912,
                    "speed_bps": 10485760,
                    "eta_seconds": 51,
                    "progress_pct": 50.0,
                    "connections_active": 8,
                    "created_at": 1726000000,
                    "started_at": 1726000005,
                    "completed_at": null,
                    "error": null
                }
            ]
        }
        """;

        var response = JsonSerializer.Deserialize<DownloadListResponse>(json);
        Assert.NotNull(response);
        Assert.Equal(1, response.Total);
        Assert.Single(response.Items);

        var item = response.Items[0];
        Assert.Equal("11111111-2222-3333-4444-555555555555", item.Id);
        Assert.Equal("test.iso", item.Filename);
        Assert.Equal((ulong)1073741824, item.TotalBytes);
        Assert.Equal((ulong)536870912, item.BytesDone);
        Assert.Equal(50.0, item.ProgressPct);
    }

    [Fact]
    public void HealthResponse_DeserializesProperly()
    {
        string json = """
        {
            "status": "ok",
            "api_version": "1.0",
            "daemon_version": "0.1.0"
        }
        """;

        var health = JsonSerializer.Deserialize<HealthResponse>(json);
        Assert.NotNull(health);
        Assert.Equal("ok", health.Status);
        Assert.Equal("1.0", health.ApiVersion);
        Assert.Equal("0.1.0", health.DaemonVersion);
    }

    [Theory]
    [InlineData("failed to connect to host: connection refused", "Couldn't connect to server")]
    [InlineData("error sending request for url: No connection could be made because the target machine actively refused it.", "Couldn't connect to server")]
    [InlineData("HTTP 404 Not Found", "File not found (404)")]
    [InlineData("server returned 403 Forbidden", "Access denied (403)")]
    [InlineData("HTTP 500 Internal Server Error", "Server error")]
    [InlineData("HTTP 502 Bad Gateway", "Server error")]
    [InlineData("operation timed out after 30s", "Connection timed out")]
    [InlineData("there is not enough space on disk", "Disk full")]
    [InlineData("SSL certificate verification failed", "SSL certificate error")]
    [InlineData("Access is denied", "Permission denied")]
    public void DownloadItem_FriendlyError_MapsTechnicalErrorsToCleanUserSummaries(string rawError, string expectedFriendly)
    {
        var item = new DownloadItem
        {
            Status = DownloadStatus.Failed,
            Error = rawError
        };

        Assert.Equal(expectedFriendly, item.FriendlyError);
        Assert.Equal(expectedFriendly, item.ShortError);
        Assert.Contains(rawError, item.ErrorToolTip);
    }
}
