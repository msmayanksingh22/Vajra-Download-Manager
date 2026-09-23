using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using Vajra.Windows.Models;

namespace Vajra.Windows.Services;

public class SseService : ISseService, IDisposable
{
    private readonly ITokenService _tokenService;
    private readonly HttpClient _httpClient;
    private readonly object _lifecycleLock = new();
    private CancellationTokenSource? _cts;
    private Task? _listenerTask;
    private bool _isConnected;
    private bool _isDisposed;

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    public event Action<SseProgressPayload>? OnProgress;
    public event Action<SseBatchProgressPayload>? OnBatchProgress;
    public event Action<SseStateChangePayload>? OnStateChange;
    public event Action<SseAddedPayload>? OnAdded;
    public event Action<SseRemovedPayload>? OnRemoved;
    public event Action? OnConnected;
    public event Action? OnDisconnected;
    public event Action? OnAuthenticationFailed;

    public bool IsConnected => _isConnected;

    public SseService(ITokenService tokenService, HttpClient? httpClient = null)
    {
        _tokenService = tokenService ?? throw new ArgumentNullException(nameof(tokenService));
        _httpClient = httpClient ?? new HttpClient { Timeout = Timeout.InfiniteTimeSpan };
    }

    public Task StartAsync(CancellationToken ct = default)
    {
        lock (_lifecycleLock)
        {
            if (_isDisposed) throw new ObjectDisposedException(nameof(SseService));
            if (_listenerTask != null && !_listenerTask.IsCompleted)
            {
                return Task.CompletedTask;
            }

            _cts?.Dispose();
            _cts = ct.CanBeCanceled ? CancellationTokenSource.CreateLinkedTokenSource(ct) : new CancellationTokenSource();
            var token = _cts.Token;
            _listenerTask = Task.Run(() => ListenLoopAsync(token), token);
            return Task.CompletedTask;
        }
    }

    public async Task StopAsync()
    {
        Task? taskToWait = null;
        CancellationTokenSource? ctsToDispose = null;

        lock (_lifecycleLock)
        {
            if (_cts != null)
            {
                _cts.Cancel();
                ctsToDispose = _cts;
                _cts = null;
            }

            // Prevent self-await deadlock if StopAsync is called from inside an event handler
            if (_listenerTask != null && _listenerTask.Id != Task.CurrentId)
            {
                taskToWait = _listenerTask;
            }
            _listenerTask = null;
        }

        if (taskToWait != null)
        {
            try
            {
                await taskToWait;
            }
            catch
            {
                // ignore cancellation exception
            }
        }

        ctsToDispose?.Dispose();
        SetConnected(false);
    }

    private async Task ListenLoopAsync(CancellationToken ct)
    {
        int currentDelayMs = 1000;
        const int maxDelayMs = 15000;

        try
        {
            while (!ct.IsCancellationRequested)
            {
                bool isAuthFailure = false;

                try
                {
                    string? token = _tokenService.GetToken();
                    string baseUrl = _tokenService.GetBaseUrl();
                    // GLM Fix: Never send token in query string. Use Authorization header only.
                    string url = $"{baseUrl}/api/v1/events";

                    using var request = new HttpRequestMessage(HttpMethod.Get, url);
                    if (!string.IsNullOrEmpty(token))
                    {
                        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
                    }

                    using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, ct);

                    if (response.StatusCode == HttpStatusCode.Unauthorized)
                    {
                        isAuthFailure = true;
                        SetConnected(false);
                        OnAuthenticationFailed?.Invoke();
                        currentDelayMs = maxDelayMs; // Avoid aggressive polling when unauthorized
                    }
                    else if (response.IsSuccessStatusCode)
                    {
                        SetConnected(true);
                        currentDelayMs = 1000; // Reset backoff upon successful connection

                        using var stream = await response.Content.ReadAsStreamAsync(ct);
                        using var reader = new StreamReader(stream, Encoding.UTF8);

                        string? eventType = null;
                        var dataBuilder = new StringBuilder();

                        while (!reader.EndOfStream && !ct.IsCancellationRequested)
                        {
                            string? line = await reader.ReadLineAsync(ct);
                            if (line == null) break;

                            if (line.StartsWith("event:"))
                            {
                                eventType = line.Substring(6).Trim();
                            }
                            else if (line.StartsWith("data:"))
                            {
                                dataBuilder.AppendLine(line.Substring(5).Trim());
                            }
                            else if (line.StartsWith(":"))
                            {
                                // SSE comment / keep-alive ping
                            }
                            else if (string.IsNullOrWhiteSpace(line))
                            {
                                if (dataBuilder.Length > 0)
                                {
                                    string data = dataBuilder.ToString().Trim();
                                    DispatchEvent(eventType ?? "message", data);
                                    dataBuilder.Clear();
                                    eventType = null;
                                }
                            }
                        }
                    }
                }
                catch (OperationCanceledException)
                {
                    break;
                }
                catch
                {
                    // Network error or daemon offline
                }

                SetConnected(false);

                if (!ct.IsCancellationRequested)
                {
                    try
                    {
                        await Task.Delay(currentDelayMs, ct);
                        if (!isAuthFailure)
                        {
                            currentDelayMs = Math.Min(currentDelayMs * 2, maxDelayMs);
                        }
                    }
                    catch (OperationCanceledException)
                    {
                        break;
                    }
                }
            }
        }
        finally
        {
            SetConnected(false);
            lock (_lifecycleLock)
            {
                if (_cts != null && ct.IsCancellationRequested && (_listenerTask == null || _listenerTask.IsCompleted))
                {
                    _cts.Dispose();
                    _cts = null;
                    _listenerTask = null;
                }
            }
        }
    }

    private void SetConnected(bool connected)
    {
        if (_isConnected != connected)
        {
            _isConnected = connected;
            if (_isConnected)
            {
                OnConnected?.Invoke();
            }
            else
            {
                OnDisconnected?.Invoke();
            }
        }
    }

    private void DispatchEvent(string eventType, string data)
    {
        try
        {
            switch (eventType)
            {
                case "progress":
                {
                    var payload = JsonSerializer.Deserialize<SseProgressPayload>(data, JsonOptions);
                    if (payload != null) OnProgress?.Invoke(payload);
                    break;
                }
                case "batch_progress":
                {
                    var payload = JsonSerializer.Deserialize<SseBatchProgressPayload>(data, JsonOptions);
                    if (payload != null) OnBatchProgress?.Invoke(payload);
                    break;
                }
                case "state_change":
                {
                    var payload = JsonSerializer.Deserialize<SseStateChangePayload>(data, JsonOptions);
                    if (payload != null) OnStateChange?.Invoke(payload);
                    break;
                }
                case "added":
                {
                    var payload = JsonSerializer.Deserialize<SseAddedPayload>(data, JsonOptions);
                    if (payload != null) OnAdded?.Invoke(payload);
                    break;
                }
                case "removed":
                {
                    var payload = JsonSerializer.Deserialize<SseRemovedPayload>(data, JsonOptions);
                    if (payload != null) OnRemoved?.Invoke(payload);
                    break;
                }
            }
        }
        catch
        {
            // Ignore deserialization errors for unknown/malformed events
        }
    }

    public void Dispose()
    {
        lock (_lifecycleLock)
        {
            if (_isDisposed) return;
            _isDisposed = true;
            _cts?.Cancel();
            _cts?.Dispose();
            _cts = null;
            _listenerTask = null;
        }
        SetConnected(false);
    }
}
