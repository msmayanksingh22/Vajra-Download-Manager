using System.Text.Json;
using System.Text.Json.Serialization;

namespace Vajra.Windows.Models;

public class DownloadListResponse
{
    [JsonPropertyName("total")]
    public int Total { get; set; }

    [JsonPropertyName("limit")]
    public int Limit { get; set; }

    [JsonPropertyName("offset")]
    public int Offset { get; set; }

    [JsonPropertyName("items")]
    public List<DownloadInfoResponse> Items { get; set; } = new();
}

public class DownloadInfoResponse
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; set; } = "idle";

    [JsonPropertyName("url")]
    public string Url { get; set; } = string.Empty;

    [JsonPropertyName("output_path")]
    public string? OutputPath { get; set; }

    [JsonPropertyName("filename")]
    public string Filename { get; set; } = string.Empty;

    [JsonPropertyName("total_bytes")]
    public ulong? TotalBytes { get; set; }

    [JsonPropertyName("bytes_done")]
    public ulong BytesDone { get; set; }

    [JsonPropertyName("speed_bps")]
    public ulong SpeedBps { get; set; }

    [JsonPropertyName("eta_seconds")]
    public ulong? EtaSeconds { get; set; }

    [JsonPropertyName("progress_pct")]
    public double ProgressPct { get; set; }

    [JsonPropertyName("connections_active")]
    public byte ConnectionsActive { get; set; }

    [JsonPropertyName("created_at")]
    public long CreatedAt { get; set; }

    [JsonPropertyName("started_at")]
    public long? StartedAt { get; set; }

    [JsonPropertyName("completed_at")]
    public long? CompletedAt { get; set; }

    [JsonPropertyName("error")]
    public string? Error { get; set; }

    [JsonPropertyName("segments")]
    public List<SegmentModel> Segments { get; set; } = new();

    [JsonPropertyName("expected_hash")]
    public string? ExpectedHash { get; set; }

    [JsonPropertyName("actual_hash")]
    public string? ActualHash { get; set; }

    [JsonPropertyName("hash_algorithm")]
    public string? HashAlgorithm { get; set; }

    [JsonPropertyName("resume_supported")]
    public bool ResumeSupported { get; set; }
}

public class BulkActionRequest
{
    [JsonPropertyName("ids")]
    public List<string> Ids { get; set; } = new();

    [JsonPropertyName("action")]
    public string Action { get; set; } = string.Empty;

    [JsonPropertyName("all")]
    public bool All { get; set; }

    [JsonPropertyName("delete_file")]
    public bool DeleteFile { get; set; }
}

public class BulkActionResponse
{
    [JsonPropertyName("total")]
    public int Total { get; set; }

    [JsonPropertyName("succeeded")]
    public List<string> Succeeded { get; set; } = new();

    [JsonPropertyName("failed")]
    public List<BulkActionFailure> Failed { get; set; } = new();
}

public class BulkActionFailure
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;

    [JsonPropertyName("code")]
    public string Code { get; set; } = string.Empty;

    [JsonPropertyName("message")]
    public string Message { get; set; } = string.Empty;

    [JsonIgnore]
    public string Error => !string.IsNullOrEmpty(Message) ? Message : Code;
}

public class StringOrNumberConverter : JsonConverter<string?>
{
    public override string? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Number)
        {
            return reader.TryGetInt64(out long l) ? l.ToString() : reader.GetDouble().ToString();
        }
        if (reader.TokenType == JsonTokenType.String)
        {
            return reader.GetString();
        }
        return null;
    }

    public override void Write(Utf8JsonWriter writer, string? value, JsonSerializerOptions options)
    {
        writer.WriteStringValue(value);
    }
}

public class HealthResponse
{
    [JsonPropertyName("status")]
    public string Status { get; set; } = string.Empty;

    [JsonPropertyName("api_version")]
    [JsonConverter(typeof(StringOrNumberConverter))]
    public string? ApiVersion { get; set; }

    [JsonPropertyName("daemon_version")]
    public string? DaemonVersion { get; set; }
}

public class AddDownloadRequest
{
    [JsonPropertyName("url")]
    public string Url { get; set; } = string.Empty;

    [JsonPropertyName("filename")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Filename { get; set; }

    [JsonPropertyName("output_dir")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? OutputDir { get; set; }
}

public class AddDownloadResponse
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;

    [JsonPropertyName("status")]
    public string Status { get; set; } = string.Empty;

    [JsonPropertyName("filename")]
    public string? Filename { get; set; }
}

public class PatchDownloadRequest
{
    [JsonPropertyName("action")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Action { get; set; }

    [JsonPropertyName("filename")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Filename { get; set; }

    [JsonPropertyName("url")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Url { get; set; }
}
