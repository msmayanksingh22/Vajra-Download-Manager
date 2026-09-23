using System.Text.Json;
using Vajra.Windows.Models;
using Xunit;

namespace Vajra.Windows.Tests;

public class DaemonEventParsingTests
{
    [Fact]
    public void ProgressPayload_DeserializesAccurately()
    {
        string json = """
        {
            "download_id": "99999999-8888-7777-6666-555555555555",
            "url": "https://mirror.example.com/file.tar.gz",
            "filename": "file.tar.gz",
            "total_bytes": 50000000,
            "downloaded_bytes": 25000000,
            "speed_bps": 2048000,
            "eta_seconds": 12,
            "status": "downloading",
            "resume_supported": true,
            "error": null
        }
        """;

        var payload = JsonSerializer.Deserialize<SseProgressPayload>(json);
        Assert.NotNull(payload);
        Assert.Equal("99999999-8888-7777-6666-555555555555", payload.DownloadId);
        Assert.Equal("file.tar.gz", payload.Filename);
        Assert.Equal((ulong)50000000, payload.TotalBytes);
        Assert.Equal((ulong)25000000, payload.DownloadedBytes);
        Assert.Equal((ulong)2048000, payload.SpeedBps);
        Assert.Equal((ulong)12, payload.EtaSeconds);
        Assert.Equal("downloading", payload.Status);
        Assert.True(payload.ResumeSupported);
    }

    [Fact]
    public void StateChangePayload_DeserializesAccurately()
    {
        string json = """
        {
            "id": "12345678-1234-1234-1234-123456789abc",
            "status": "completed",
            "output_path": "C:\\Downloads\\completed_file.mp4",
            "error": null
        }
        """;

        var payload = JsonSerializer.Deserialize<SseStateChangePayload>(json);
        Assert.NotNull(payload);
        Assert.Equal("12345678-1234-1234-1234-123456789abc", payload.Id);
        Assert.Equal("completed", payload.Status);
        Assert.Equal("C:\\Downloads\\completed_file.mp4", payload.OutputPath);
        Assert.Null(payload.Error);
    }

    [Fact]
    public void AddedAndRemovedPayloads_DeserializeAccurately()
    {
        string addedJson = """
        {
            "id": "aaaa-bbbb-cccc-dddd",
            "url": "https://test.com/new.zip",
            "filename": "new.zip"
        }
        """;

        var added = JsonSerializer.Deserialize<SseAddedPayload>(addedJson);
        Assert.NotNull(added);
        Assert.Equal("aaaa-bbbb-cccc-dddd", added.Id);
        Assert.Equal("new.zip", added.Filename);

        string removedJson = """
        {
            "id": "aaaa-bbbb-cccc-dddd"
        }
        """;

        var removed = JsonSerializer.Deserialize<SseRemovedPayload>(removedJson);
        Assert.NotNull(removed);
        Assert.Equal("aaaa-bbbb-cccc-dddd", removed.Id);
    }

    [Fact]
    public void BatchProgressPayload_DeserializesAccurately()
    {
        string json = """
        {
            "downloads": [
                {
                    "download_id": "item-1",
                    "url": "https://test.com/1",
                    "filename": "1.bin",
                    "total_bytes": 100,
                    "downloaded_bytes": 50,
                    "speed_bps": 10,
                    "eta_seconds": 5,
                    "status": "downloading",
                    "resume_supported": true
                },
                {
                    "download_id": "item-2",
                    "url": "https://test.com/2",
                    "filename": "2.bin",
                    "total_bytes": 200,
                    "downloaded_bytes": 200,
                    "speed_bps": 0,
                    "eta_seconds": 0,
                    "status": "completed",
                    "resume_supported": true
                }
            ]
        }
        """;

        var batch = JsonSerializer.Deserialize<SseBatchProgressPayload>(json);
        Assert.NotNull(batch);
        Assert.Equal(2, batch.Downloads.Count);
        Assert.Equal("item-1", batch.Downloads[0].DownloadId);
        Assert.Equal("item-2", batch.Downloads[1].DownloadId);
    }
}
