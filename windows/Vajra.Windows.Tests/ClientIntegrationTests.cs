using System.Net;
using System.Net.Http;
using System.Text;
using Vajra.Windows.Models;
using Vajra.Windows.Services;
using Xunit;

namespace Vajra.Windows.Tests;

public class MockHttpMessageHandler : HttpMessageHandler
{
    public Func<HttpRequestMessage, HttpResponseMessage>? Handler { get; set; }
    public Func<HttpRequestMessage, Task<HttpResponseMessage>>? AsyncHandler { get; set; }
    public HttpRequestMessage? LastRequest { get; private set; }

    protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        LastRequest = request;
        if (AsyncHandler != null)
        {
            return await AsyncHandler(request);
        }
        return Handler != null ? Handler(request) : new HttpResponseMessage(HttpStatusCode.OK);
    }
}

public class MockTokenService : ITokenService
{
    public string? Token { get; set; } = "mock_secret_token_1234567890abcdef";
    public string BaseUrl { get; set; } = "http://127.0.0.1:6277";
    public ushort Port { get; set; } = 6277;

    public string? GetToken() => Token;
    public string GetBaseUrl() => BaseUrl;
    public ushort GetPort() => Port;
    public bool HasToken() => !string.IsNullOrEmpty(Token);
    public void InvalidateCache() { }
}

public class ClientIntegrationTests
{
    [Fact]
    public async Task DaemonClient_AppliesBearerTokenToRequests()
    {
        var mockHandler = new MockHttpMessageHandler
        {
            Handler = req => new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent("{\"total\":0,\"limit\":50,\"offset\":0,\"items\":[]}", Encoding.UTF8, "application/json")
            }
        };

        var tokenService = new MockTokenService { Token = "test_bearer_token_123456789" };
        var httpClient = new HttpClient(mockHandler);
        var client = new DaemonClient(tokenService, httpClient);

        var downloads = await client.GetDownloadsAsync();
        Assert.NotNull(downloads);

        Assert.NotNull(mockHandler.LastRequest);
        Assert.NotNull(mockHandler.LastRequest.Headers.Authorization);
        Assert.Equal("Bearer", mockHandler.LastRequest.Headers.Authorization.Scheme);
        Assert.Equal("test_bearer_token_123456789", mockHandler.LastRequest.Headers.Authorization.Parameter);
    }

    [Fact]
    public async Task DaemonClient_AddDownload_SendsCorrectJsonPayload()
    {
        string? capturedBody = null;
        var mockHandler = new MockHttpMessageHandler
        {
            AsyncHandler = async req =>
            {
                capturedBody = req.Content != null ? await req.Content.ReadAsStringAsync() : null;
                return new HttpResponseMessage(HttpStatusCode.Created)
                {
                    Content = new StringContent("{\"id\":\"test-id-123\",\"status\":\"queued\",\"filename\":\"test.zip\"}", Encoding.UTF8, "application/json")
                };
            }
        };

        var tokenService = new MockTokenService();
        var httpClient = new HttpClient(mockHandler);
        var client = new DaemonClient(tokenService, httpClient);

        var resp = await client.AddDownloadAsync("https://example.com/file.zip", "custom.zip");
        Assert.NotNull(resp);
        Assert.Equal("test-id-123", resp.Id);
        Assert.Equal("queued", resp.Status);

        Assert.NotNull(capturedBody);
        Assert.Contains("\"url\":\"https://example.com/file.zip\"", capturedBody);
        Assert.Contains("\"filename\":\"custom.zip\"", capturedBody);
    }

    [Fact]
    public async Task DaemonClient_PauseAndResume_SendExpectedActions()
    {
        string? capturedAction = null;
        var mockHandler = new MockHttpMessageHandler
        {
            AsyncHandler = async req =>
            {
                capturedAction = req.Content != null ? await req.Content.ReadAsStringAsync() : null;
                return new HttpResponseMessage(HttpStatusCode.OK);
            }
        };

        var tokenService = new MockTokenService();
        var httpClient = new HttpClient(mockHandler);
        var client = new DaemonClient(tokenService, httpClient);

        bool pauseOk = await client.PauseDownloadAsync("item-1");
        Assert.True(pauseOk);
        Assert.Contains("\"action\":\"pause\"", capturedAction);

        bool resumeOk = await client.ResumeDownloadAsync("item-1");
        Assert.True(resumeOk);
        Assert.Contains("\"action\":\"resume\"", capturedAction);
    }

    [Fact]
    public async Task DaemonClient_Delete_SendsDeleteParam()
    {
        string? capturedUrl = null;
        var mockHandler = new MockHttpMessageHandler
        {
            Handler = req =>
            {
                capturedUrl = req.RequestUri?.ToString();
                return new HttpResponseMessage(HttpStatusCode.OK);
            }
        };

        var tokenService = new MockTokenService();
        var httpClient = new HttpClient(mockHandler);
        var client = new DaemonClient(tokenService, httpClient);

        bool delOk = await client.DeleteDownloadAsync("item-99", deleteFile: true);
        Assert.True(delOk);
        Assert.NotNull(capturedUrl);
        Assert.Contains("/api/v1/downloads/item-99?delete_file=true", capturedUrl);
    }

    [Fact]
    public async Task DaemonClient_GracefullyHandlesNetworkFailure()
    {
        var mockHandler = new MockHttpMessageHandler
        {
            Handler = _ => throw new HttpRequestException("Connection refused")
        };

        var tokenService = new MockTokenService();
        var httpClient = new HttpClient(mockHandler);
        var client = new DaemonClient(tokenService, httpClient);

        var health = await client.CheckHealthAsync();
        Assert.Null(health);

        var downloads = await client.GetDownloadsAsync();
        Assert.NotNull(downloads);
        Assert.Empty(downloads);

        var add = await client.AddDownloadAsync("https://example.com/test");
        Assert.Null(add);

        bool pause = await client.PauseDownloadAsync("id-1");
        Assert.False(pause);
    }
}
