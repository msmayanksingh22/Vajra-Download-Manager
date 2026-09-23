namespace Vajra.Windows.Services;

public interface ISystemInteractionService
{
    bool OpenFile(string path);
    bool ShowInFolder(string path);
    void CopyToClipboard(string text);
    string? GetClipboardText();
}
