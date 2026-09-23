using System.IO;
using System.Net;
using System.Net.Http;
using System.Text;
using Vajra.Windows.Models;
using Vajra.Windows.Services;
using Xunit;

namespace Vajra.Windows.Tests;

public class SseMockHttpHandler : HttpMessageHandler
{
    private readonly string _sseContent;

    public SseMockHttpHandler(string sseContent)
    {
        _sseContent = sseContent;
    }

    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        var response = new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new StringContent(_sseContent, Encoding.UTF8, "text/event-stream")
        };
        return Task.FromResult(response);
    }
}

public class SseServiceTests
{
    [Fact]
    public async Task SseService_ParsesAndDispatchesProgressEvent()
    {
        string sseStream = "event: progress\n" +
                           "data: {\"download_id\":\"dl-1\",\"url\":\"https://example.com/file\",\"filename\":\"file.bin\",\"downloaded_bytes\":100,\"total_bytes\":200,\"speed_bps\":50,\"eta_seconds\":2,\"status\":\"downloading\",\"resume_supported\":true}\n\n";

        var tokenService = new MockTokenService();
        var handler = new SseMockHttpHandler(sseStream);
        var httpClient = new HttpClient(handler);
        var sseService = new SseService(tokenService, httpClient);

        var tcs = new TaskCompletionSource<SseProgressPayload>();
        sseService.OnProgress += payload => tcs.TrySetResult(payload);

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        await sseService.StartAsync(cts.Token);

        var received = await tcs.Task;
        Assert.NotNull(received);
        Assert.Equal("dl-1", received.DownloadId);
        Assert.Equal("file.bin", received.Filename);
        Assert.Equal((ulong)100, received.DownloadedBytes);
        Assert.Equal((ulong)200, received.TotalBytes);

        await sseService.StopAsync();
    }

    [Fact]
    public async Task SseService_ParsesAndDispatchesStateChangeEvent()
    {
        string sseStream = "event: state_change\n" +
                           "data: {\"id\":\"dl-2\",\"status\":\"completed\",\"output_path\":\"C:\\\\Downloads\\\\done.mp4\"}\n\n";

        var tokenService = new MockTokenService();
        var handler = new SseMockHttpHandler(sseStream);
        var httpClient = new HttpClient(handler);
        var sseService = new SseService(tokenService, httpClient);

        var tcs = new TaskCompletionSource<SseStateChangePayload>();
        sseService.OnStateChange += payload => tcs.TrySetResult(payload);

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        await sseService.StartAsync(cts.Token);

        var received = await tcs.Task;
        Assert.NotNull(received);
        Assert.Equal("dl-2", received.Id);
        Assert.Equal("completed", received.Status);
        Assert.Equal("C:\\Downloads\\done.mp4", received.OutputPath);

        await sseService.StopAsync();
    }

    [Fact]
    public async Task SseService_UsesAuthorizationHeaderAndNoQueryToken()
    {
        string? capturedAuthHeader = null;
        string? capturedQuery = null;

        var delegatingHandler = new DelegatingMockHandler(req =>
        {
            capturedAuthHeader = req.Headers.Authorization?.ToString();
            capturedQuery = req.RequestUri?.Query;
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(": ping\n\n", Encoding.UTF8, "text/event-stream")
            };
        });

        var tokenService = new MockTokenService { Token = "secure_token_1234567890abcdef" };
        var httpClient = new HttpClient(delegatingHandler);
        using var sseService = new SseService(tokenService, httpClient);

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(2));
        await sseService.StartAsync(cts.Token);
        await Task.Delay(200);
        await sseService.StopAsync();

        Assert.Equal("Bearer secure_token_1234567890abcdef", capturedAuthHeader);
        Assert.True(string.IsNullOrEmpty(capturedQuery), "SSE URL should not contain credentials in query string.");
    }

    [Fact]
    public async Task SseService_InvokesOnAuthenticationFailed_On401()
    {
        var delegatingHandler = new DelegatingMockHandler(req =>
        {
            return new HttpResponseMessage(HttpStatusCode.Unauthorized);
        });

        var tokenService = new MockTokenService { Token = "bad_token_1234567890abcdef" };
        var httpClient = new HttpClient(delegatingHandler);
        using var sseService = new SseService(tokenService, httpClient);

        var authFailedTcs = new TaskCompletionSource<bool>();
        sseService.OnAuthenticationFailed += () => authFailedTcs.TrySetResult(true);

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(2));
        await sseService.StartAsync(cts.Token);

        var completed = await Task.WhenAny(authFailedTcs.Task, Task.Delay(1000));
        Assert.True(completed == authFailedTcs.Task, "OnAuthenticationFailed should fire when daemon returns 401.");

        await sseService.StopAsync();
    }

    [Fact]
    public async Task SseService_CanStopFromInsideEventHandler_WithoutDeadlock()
    {
        string sseStream = "event: progress\n" +
                           "data: {\"download_id\":\"dl-self-stop\",\"downloaded_bytes\":50,\"total_bytes\":100,\"speed_bps\":10,\"status\":\"downloading\"}\n\n";

        var tokenService = new MockTokenService();
        var handler = new SseMockHttpHandler(sseStream);
        var httpClient = new HttpClient(handler);
        using var sseService = new SseService(tokenService, httpClient);

        var stoppedTcs = new TaskCompletionSource<bool>();

        sseService.OnProgress += async (payload) =>
        {
            // Stopping from inside event handler must not deadlock
            await sseService.StopAsync();
            stoppedTcs.TrySetResult(true);
        };

        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(3));
        await sseService.StartAsync(cts.Token);

        var completed = await Task.WhenAny(stoppedTcs.Task, Task.Delay(2000));
        Assert.True(completed == stoppedTcs.Task, "StopAsync called from event handler should complete without deadlocking.");
        Assert.False(sseService.IsConnected);
    }
}

public class DelegatingMockHandler : HttpMessageHandler
{
    private readonly Func<HttpRequestMessage, HttpResponseMessage> _handler;
    public DelegatingMockHandler(Func<HttpRequestMessage, HttpResponseMessage> handler) => _handler = handler;
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        return Task.FromResult(_handler(request));
    }
}
