using System.Diagnostics;
using System.IO;
using System.Windows;

namespace Vajra.Windows.Services;

public class SystemInteractionService : ISystemInteractionService
{
    public bool OpenFile(string path)
    {
        if (string.IsNullOrWhiteSpace(path)) return false;

        try
        {
            if (!File.Exists(path)) return false;

            Process.Start(new ProcessStartInfo
            {
                FileName = path,
                UseShellExecute = true
            });
            return true;
        }
        catch
        {
            return false;
        }
    }

    public bool ShowInFolder(string path)
    {
        if (string.IsNullOrWhiteSpace(path)) return false;

        try
        {
            string cleanPath = path.Replace('/', '\\');
            if (File.Exists(cleanPath))
            {
                Process.Start("explorer.exe", $"/select,\"{cleanPath}\"");
                return true;
            }

            string? dir = Path.GetDirectoryName(cleanPath);
            if (!string.IsNullOrEmpty(dir) && Directory.Exists(dir))
            {
                Process.Start("explorer.exe", $"\"{dir}\"");
                return true;
            }

            return false;
        }
        catch
        {
            return false;
        }
    }

    public void CopyToClipboard(string text)
    {
        try
        {
            if (!string.IsNullOrEmpty(text))
            {
                Clipboard.SetText(text);
            }
        }
        catch
        {
            // Transient clipboard locking by other applications
        }
    }

    public string? GetClipboardText()
    {
        try
        {
            if (Clipboard.ContainsText())
            {
                return Clipboard.GetText();
            }
        }
        catch
        {
            // Clipboard access exception
        }
        return null;
    }
}
