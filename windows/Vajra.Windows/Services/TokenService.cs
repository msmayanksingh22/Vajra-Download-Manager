using System.IO;
using System.Text.Json;

namespace Vajra.Windows.Services;

public class TokenService : ITokenService
{
    public const ushort DefaultPort = 6277;
    private string? _cachedToken;
    private ushort? _cachedPort;

    public string? GetToken()
    {
        if (_cachedToken != null) return _cachedToken;

        // 1. Environment variable override
        string? envToken = Environment.GetEnvironmentVariable("VAJRA_API_TOKEN");
        if (IsValidToken(envToken))
        {
            _cachedToken = envToken!.Trim();
            return _cachedToken;
        }

        // 2. Authoritative config token: config.json explicit api_token (supersedes api.token file)
        string configPath = GetConfigPath();
        if (File.Exists(configPath))
        {
            try
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(configPath));
                if (doc.RootElement.TryGetProperty("api_token", out var tokenProp))
                {
                    string? cfgTok = tokenProp.GetString();
                    if (IsValidToken(cfgTok))
                    {
                        _cachedToken = cfgTok!.Trim();
                        return _cachedToken;
                    }
                }
            }
            catch
            {
                // ignore config parse issues
            }
        }

        // 3. Token file on disk: %LOCALAPPDATA%\Vajra\api.token or VAJRA_DATA_DIR/api.token
        string tokenPath = GetTokenPath();
        if (File.Exists(tokenPath))
        {
            try
            {
                string content = File.ReadAllText(tokenPath).Trim();
                if (IsValidToken(content))
                {
                    _cachedToken = content;
                    return _cachedToken;
                }
            }
            catch
            {
                // transient read error
            }
        }

        return null;
    }

    public ushort GetPort()
    {
        if (_cachedPort.HasValue) return _cachedPort.Value;

        // 1. VAJRA_PORT env var
        string? envPort = Environment.GetEnvironmentVariable("VAJRA_PORT");
        if (!string.IsNullOrEmpty(envPort) && ushort.TryParse(envPort, out ushort parsedPort) && parsedPort > 0)
        {
            _cachedPort = parsedPort;
            return parsedPort;
        }

        // 2. config.json listen_port
        string configPath = GetConfigPath();
        if (File.Exists(configPath))
        {
            try
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(configPath));
                if (doc.RootElement.TryGetProperty("listen_port", out var portProp) && portProp.TryGetUInt16(out ushort cfgPort) && cfgPort > 0)
                {
                    _cachedPort = cfgPort;
                    return cfgPort;
                }
            }
            catch
            {
                // ignore
            }
        }

        _cachedPort = DefaultPort;
        return DefaultPort;
    }

    public string GetBaseUrl()
    {
        return $"http://127.0.0.1:{GetPort()}";
    }

    public bool HasToken()
    {
        return !string.IsNullOrEmpty(GetToken());
    }

    public void InvalidateCache()
    {
        _cachedToken = null;
        _cachedPort = null;
    }

    public static string GetDataDir()
    {
        string? dataDir = Environment.GetEnvironmentVariable("VAJRA_DATA_DIR");
        if (!string.IsNullOrWhiteSpace(dataDir))
        {
            return dataDir;
        }

        string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return Path.Combine(localAppData, "Vajra");
    }

    public static string GetTokenPath()
    {
        return Path.Combine(GetDataDir(), "api.token");
    }

    public static string GetConfigPath()
    {
        return Path.Combine(GetDataDir(), "config.json");
    }

    public static bool IsValidToken(string? token)
    {
        if (string.IsNullOrWhiteSpace(token)) return false;
        string t = token.Trim();
        if (t.Length < 16) return false;
        if (t == "********" || t.All(c => c == '*')) return false;
        return true;
    }
}
