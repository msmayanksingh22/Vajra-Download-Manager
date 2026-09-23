using System.Text.Json.Serialization;

namespace Vajra.Windows.Models;

public class SseProgressPayload
{
    [JsonPropertyName("download_id")]
    public string DownloadId { get; set; } = string.Empty;

    [JsonPropertyName("url")]
    public string Url { get; set; } = string.Empty;

    [JsonPropertyName("filename")]
    public string Filename { get; set; } = string.Empty;

    [JsonPropertyName("total_bytes")]
    public ulong? TotalBytes { get; set; }

    [JsonPropertyName("downloaded_bytes")]
    public ulong DownloadedBytes { get; set; }

    [JsonPropertyName("speed_bps")]
    public ulong SpeedBps { get; set; }

    [JsonPropertyName("eta_seconds")]
    public ulong? EtaSeconds { get; set; }

    [JsonPropertyName("status")]
    public string Status { get; set; } = string.Empty;

    [JsonPropertyName("resume_supported")]
    public bool ResumeSupported { get; set; }

    [JsonPropertyName("segments")]
    public List<SegmentModel> Segments { get; set; } = new();

    [JsonPropertyName("error")]
    public string? Error { get; set; }
}

public class SegmentModel
{
    [JsonPropertyName("id")]
    public int Id { get; set; }

    [JsonPropertyName("start")]
    public ulong Start { get; set; }

    [JsonPropertyName("end")]
    public ulong End { get; set; }

    [JsonPropertyName("bytes_done")]
    public ulong BytesDone { get; set; }

    [JsonPropertyName("allocated_bytes")]
    public ulong AllocatedBytes { get; set; }

    [JsonPropertyName("status")]
    public string Status { get; set; } = string.Empty;

    [JsonPropertyName("thread_index")]
    public int ThreadIndex { get; set; }

    [JsonPropertyName("speed_bps")]
    public ulong? SpeedBps { get; set; }

    [JsonPropertyName("retry_count")]
    public int RetryCount { get; set; }

    [JsonPropertyName("error_message")]
    public string? ErrorMessage { get; set; }

    public double ProgressPct => (End >= Start && (End - Start) > 0)
        ? Math.Clamp((double)BytesDone / (double)(End - Start) * 100.0, 0.0, 100.0)
        : (BytesDone > 0 ? 100.0 : 0.0);

    public string ProgressPctFormatted => $"{ProgressPct:F1}%";

    public string RangeFormatted => $"{Start:N0} - {End:N0}";

    public int Index => Id + 1;

    public string ProgressFormatted => $"{BytesDoneFormatted} ({ProgressPct:F1}%)";

    public string BytesDoneFormatted
    {
        get
        {
            if (BytesDone >= 1024 * 1024 * 1024)
                return $"{(double)BytesDone / (1024 * 1024 * 1024):F2} GB";
            if (BytesDone >= 1024 * 1024)
                return $"{(double)BytesDone / (1024 * 1024):F2} MB";
            if (BytesDone >= 1024)
                return $"{(double)BytesDone / 1024:F1} KB";
            return $"{BytesDone} B";
        }
    }
}

public class SseStateChangePayload
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; set; } = string.Empty;

    [JsonPropertyName("output_path")]
    public string? OutputPath { get; set; }

    [JsonPropertyName("error")]
    public string? Error { get; set; }
}

public class SseAddedPayload
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;

    [JsonPropertyName("url")]
    public string Url { get; set; } = string.Empty;

    [JsonPropertyName("filename")]
    public string Filename { get; set; } = string.Empty;
}

public class SseRemovedPayload
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;
}

public class SseBatchProgressPayload
{
    [JsonPropertyName("downloads")]
    public List<SseProgressPayload> Downloads { get; set; } = new();
}
