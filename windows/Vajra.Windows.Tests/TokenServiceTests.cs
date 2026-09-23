using System.IO;
using Vajra.Windows.Services;
using Xunit;

namespace Vajra.Windows.Tests;

public class TokenServiceTests
{
    [Fact]
    public void IsValidToken_AcceptsCryptographicTokens()
    {
        string validToken = "0123456789abcdef0123456789abcdef"; // 32 hex chars
        Assert.True(TokenService.IsValidToken(validToken));
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData(null)]
    [InlineData("short_token")]
    [InlineData("********")]
    [InlineData("****************")]
    [InlineData("********************************")]
    public void IsValidToken_RejectsPlaceholdersAndInvalidTokens(string? invalidToken)
    {
        Assert.False(TokenService.IsValidToken(invalidToken));
    }

    [Fact]
    public void TokenService_PrefersEnvironmentVariable()
    {
        string testToken = "abcdef1234567890abcdef1234567890";
        string original = Environment.GetEnvironmentVariable("VAJRA_API_TOKEN") ?? "";

        try
        {
            Environment.SetEnvironmentVariable("VAJRA_API_TOKEN", testToken);
            var svc = new TokenService();
            Assert.Equal(testToken, svc.GetToken());
            Assert.True(svc.HasToken());
        }
        finally
        {
            Environment.SetEnvironmentVariable("VAJRA_API_TOKEN", string.IsNullOrEmpty(original) ? null : original);
        }
    }

    [Fact]
    public void TokenService_ResolvesPortFromEnvironmentVariable()
    {
        string original = Environment.GetEnvironmentVariable("VAJRA_PORT") ?? "";

        try
        {
            Environment.SetEnvironmentVariable("VAJRA_PORT", "9876");
            var svc = new TokenService();
            Assert.Equal(9876, svc.GetPort());
            Assert.Equal("http://127.0.0.1:9876", svc.GetBaseUrl());
        }
        finally
        {
            Environment.SetEnvironmentVariable("VAJRA_PORT", string.IsNullOrEmpty(original) ? null : original);
        }
    }

    [Fact]
    public void TokenService_DefaultsToPort6277()
    {
        string original = Environment.GetEnvironmentVariable("VAJRA_PORT") ?? "";

        try
        {
            Environment.SetEnvironmentVariable("VAJRA_PORT", null);
            var svc = new TokenService();
            ushort port = svc.GetPort();
            // Either default 6277 or whatever is configured in local config.json
            Assert.True(port > 0);
            Assert.StartsWith("http://127.0.0.1:", svc.GetBaseUrl());
        }
        finally
        {
            Environment.SetEnvironmentVariable("VAJRA_PORT", string.IsNullOrEmpty(original) ? null : original);
        }
    }

    [Fact]
    public void TokenService_ExplicitConfigToken_SupersedesDifferentApiTokenFile()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "Vajra_TokenPrecTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);

        string originalDataDir = Environment.GetEnvironmentVariable("VAJRA_DATA_DIR") ?? "";
        string originalApiToken = Environment.GetEnvironmentVariable("VAJRA_API_TOKEN") ?? "";

        try
        {
            Environment.SetEnvironmentVariable("VAJRA_API_TOKEN", null);
            Environment.SetEnvironmentVariable("VAJRA_DATA_DIR", tempDir);

            string fileToken = "token_from_file_1111111111111111";
            string cfgToken = "token_from_config_2222222222222222";

            File.WriteAllText(Path.Combine(tempDir, "api.token"), fileToken);
            File.WriteAllText(Path.Combine(tempDir, "config.json"), $"{{\"api_token\": \"{cfgToken}\"}}");

            var svc = new TokenService();
            // Authoritative daemon order: config.json explicit api_token supersedes api.token file
            Assert.Equal(cfgToken, svc.GetToken());

            // If config.json has invalid/placeholder token, fallback to api.token file
            File.WriteAllText(Path.Combine(tempDir, "config.json"), "{\"api_token\": \"********\"}");
            svc.InvalidateCache();
            Assert.Equal(fileToken, svc.GetToken());
        }
        finally
        {
            Environment.SetEnvironmentVariable("VAJRA_API_TOKEN", string.IsNullOrEmpty(originalApiToken) ? null : originalApiToken);
            Environment.SetEnvironmentVariable("VAJRA_DATA_DIR", string.IsNullOrEmpty(originalDataDir) ? null : originalDataDir);

            try
            {
                if (Directory.Exists(tempDir)) Directory.Delete(tempDir, true);
            }
            catch { }
        }
    }
}
