namespace Vajra.Windows.Services;

public interface ITokenService
{
    string? GetToken();
    string GetBaseUrl();
    ushort GetPort();
    bool HasToken();
    void InvalidateCache();
}
