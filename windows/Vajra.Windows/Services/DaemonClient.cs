using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Vajra.Windows.Models;

namespace Vajra.Windows.Services;

public class DaemonClient : IDaemonClient
{
    private readonly HttpClient _httpClient;
    private readonly ITokenService _tokenService;
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    public DaemonClient(ITokenService tokenService, HttpClient? httpClient = null)
    {
        _tokenService = tokenService ?? throw new ArgumentNullException(nameof(tokenService));
        _httpClient = httpClient ?? new HttpClient { Timeout = TimeSpan.FromSeconds(10) };
    }

    private void ApplyAuthorization(HttpRequestMessage request)
    {
        string? token = _tokenService.GetToken();
        if (!string.IsNullOrEmpty(token))
        {
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        }
    }

    public string? LastError { get; private set; }

    public async Task<HealthResponse?> CheckHealthAsync(CancellationToken ct = default)
    {
        try
        {
            string url = $"{_tokenService.GetBaseUrl()}/health";
            using var response = await _httpClient.GetAsync(url, ct);
            if (!response.IsSuccessStatusCode)
            {
                LastError = $"HTTP {(int)response.StatusCode}: {await response.Content.ReadAsStringAsync(ct)}";
                return null;
            }

            string json = await response.Content.ReadAsStringAsync(ct);
            return JsonSerializer.Deserialize<HealthResponse>(json, JsonOptions);
        }
        catch (Exception ex)
        {
            LastError = ex.ToString();
            return null;
        }
    }

    public async Task<List<DownloadInfoResponse>> GetDownloadsAsync(CancellationToken ct = default)
    {
        try
        {
            string url = $"{_tokenService.GetBaseUrl()}/api/v1/downloads?limit=500";
            using var request = new HttpRequestMessage(HttpMethod.Get, url);
            ApplyAuthorization(request);

            using var response = await _httpClient.SendAsync(request, ct);
            if (!response.IsSuccessStatusCode)
            {
                LastError = $"GetDownloadsAsync HTTP {(int)response.StatusCode}: {await response.Content.ReadAsStringAsync(ct)}";
                return new List<DownloadInfoResponse>();
            }

            string json = await response.Content.ReadAsStringAsync(ct);
            try
            {
                var listResp = JsonSerializer.Deserialize<DownloadListResponse>(json, JsonOptions);
                return listResp?.Items ?? new List<DownloadInfoResponse>();
            }
            catch (Exception ex)
            {
                LastError = $"GetDownloadsAsync deserialize error: {ex.Message}. JSON: {json}";
                return new List<DownloadInfoResponse>();
            }
        }
        catch (Exception ex)
        {
            LastError = $"GetDownloadsAsync exception: {ex}";
            return new List<DownloadInfoResponse>();
        }
    }

    public async Task<AddDownloadResponse?> AddDownloadAsync(string url, string? filename = null, string? outputDir = null, CancellationToken ct = default)
    {
        try
        {
            string requestUrl = $"{_tokenService.GetBaseUrl()}/api/v1/downloads";
            using var request = new HttpRequestMessage(HttpMethod.Post, requestUrl);
            ApplyAuthorization(request);

            var reqBody = new AddDownloadRequest
            {
                Url = url,
                Filename = string.IsNullOrWhiteSpace(filename) ? null : filename.Trim(),
                OutputDir = string.IsNullOrWhiteSpace(outputDir) ? null : outputDir.Trim()
            };

            string jsonPayload = JsonSerializer.Serialize(reqBody, JsonOptions);
            request.Content = new StringContent(jsonPayload, Encoding.UTF8, "application/json");

            using var response = await _httpClient.SendAsync(request, ct);
            if (!response.IsSuccessStatusCode)
            {
                return null;
            }

            string responseJson = await response.Content.ReadAsStringAsync(ct);
            return JsonSerializer.Deserialize<AddDownloadResponse>(responseJson, JsonOptions);
        }
        catch
        {
            return null;
        }
    }

    public async Task<bool> PauseDownloadAsync(string id, CancellationToken ct = default)
    {
        return await PatchActionAsync(id, "pause", ct);
    }

    public async Task<bool> ResumeDownloadAsync(string id, CancellationToken ct = default)
    {
        return await PatchActionAsync(id, "resume", ct);
    }

    private async Task<bool> PatchActionAsync(string id, string action, CancellationToken ct = default)
    {
        try
        {
            string requestUrl = $"{_tokenService.GetBaseUrl()}/api/v1/downloads/{id}";
            using var request = new HttpRequestMessage(HttpMethod.Patch, requestUrl);
            ApplyAuthorization(request);

            var reqBody = new PatchDownloadRequest { Action = action };
            string jsonPayload = JsonSerializer.Serialize(reqBody, JsonOptions);
            request.Content = new StringContent(jsonPayload, Encoding.UTF8, "application/json");

            using var response = await _httpClient.SendAsync(request, ct);
            return response.IsSuccessStatusCode;
        }
        catch
        {
            return false;
        }
    }

    public async Task<bool> DeleteDownloadAsync(string id, bool deleteFile = false, CancellationToken ct = default)
    {
        try
        {
            string deleteParam = deleteFile ? "?delete_file=true" : "?delete_file=false";
            string requestUrl = $"{_tokenService.GetBaseUrl()}/api/v1/downloads/{id}{deleteParam}";
            using var request = new HttpRequestMessage(HttpMethod.Delete, requestUrl);
            ApplyAuthorization(request);

            using var response = await _httpClient.SendAsync(request, ct);
            return response.IsSuccessStatusCode;
        }
        catch
        {
            return false;
        }
    }

    public async Task<bool> RetryDownloadAsync(string id, CancellationToken ct = default)
    {
        return await PatchActionAsync(id, "resume", ct);
    }

    public async Task<BulkActionResponse?> BulkActionAsync(BulkActionRequest requestBody, CancellationToken ct = default)
    {
        try
        {
            string requestUrl = $"{_tokenService.GetBaseUrl()}/api/v1/downloads/bulk-action";
            using var request = new HttpRequestMessage(HttpMethod.Post, requestUrl);
            ApplyAuthorization(request);

            string jsonPayload = JsonSerializer.Serialize(requestBody, JsonOptions);
            request.Content = new StringContent(jsonPayload, Encoding.UTF8, "application/json");

            using var response = await _httpClient.SendAsync(request, ct);
            if (!response.IsSuccessStatusCode)
            {
                return null;
            }

            string responseJson = await response.Content.ReadAsStringAsync(ct);
            return JsonSerializer.Deserialize<BulkActionResponse>(responseJson, JsonOptions);
        }
        catch
        {
            return null;
        }
    }

    public async Task<bool> PauseAllAsync(CancellationToken ct = default)
    {
        var resp = await BulkActionAsync(new BulkActionRequest { All = true, Action = "pause" }, ct);
        return resp != null;
    }

    public async Task<bool> ResumeAllAsync(CancellationToken ct = default)
    {
        var resp = await BulkActionAsync(new BulkActionRequest { All = true, Action = "resume" }, ct);
        return resp != null;
    }

    public async Task<BulkActionResponse?> ClearCompletedAsync(CancellationToken ct = default)
    {
        return await BulkActionAsync(new BulkActionRequest { All = true, Action = "clear_completed" }, ct);
    }

    public async Task<DownloadInfoResponse?> GetDownloadDetailsAsync(string id, CancellationToken ct = default)
    {
        try
        {
            string requestUrl = $"{_tokenService.GetBaseUrl()}/api/v1/downloads/{id}";
            using var request = new HttpRequestMessage(HttpMethod.Get, requestUrl);
            ApplyAuthorization(request);

            using var response = await _httpClient.SendAsync(request, ct);
            if (!response.IsSuccessStatusCode)
            {
                return null;
            }

            string responseJson = await response.Content.ReadAsStringAsync(ct);
            return JsonSerializer.Deserialize<DownloadInfoResponse>(responseJson, JsonOptions);
        }
        catch
        {
            return null;
        }
    }
}
