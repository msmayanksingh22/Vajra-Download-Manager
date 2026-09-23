using System.IO;
using System.Windows.Input;
using Vajra.Windows.Services;

namespace Vajra.Windows.ViewModels;

public class AddDownloadViewModel : ViewModelBase
{
    private readonly IDaemonClient _daemonClient;
    private readonly ISystemInteractionService _systemService;
    private string _url = string.Empty;
    private string _filename = string.Empty;
    private string? _errorMessage;
    private bool _isSubmitting;

    public event Action<bool>? RequestClose;

    public string Url
    {
        get => _url;
        set
        {
            if (SetProperty(ref _url, value))
            {
                ErrorMessage = null;
                // Auto-fill filename from URL if empty or unchanged
                if (string.IsNullOrWhiteSpace(Filename) && !string.IsNullOrWhiteSpace(value))
                {
                    TryInferFilename(value);
                }
            }
        }
    }

    public string Filename
    {
        get => _filename;
        set => SetProperty(ref _filename, value);
    }

    public string? ErrorMessage
    {
        get => _errorMessage;
        set => SetProperty(ref _errorMessage, value);
    }

    public bool IsSubmitting
    {
        get => _isSubmitting;
        set => SetProperty(ref _isSubmitting, value);
    }

    public ICommand AddCommand { get; }
    public ICommand CancelCommand { get; }

    public AddDownloadViewModel(IDaemonClient daemonClient, ISystemInteractionService systemService)
    {
        _daemonClient = daemonClient;
        _systemService = systemService;

        AddCommand = new AsyncRelayCommand(ExecuteAddAsync, () => !IsSubmitting && !string.IsNullOrWhiteSpace(Url));
        CancelCommand = new RelayCommand(() => RequestClose?.Invoke(false));

        // Auto-detect URL from clipboard
        CheckClipboardForUrl();
    }

    private void CheckClipboardForUrl()
    {
        string? clip = _systemService.GetClipboardText()?.Trim();
        if (!string.IsNullOrEmpty(clip) &&
            (clip.StartsWith("http://", StringComparison.OrdinalIgnoreCase) ||
             clip.StartsWith("https://", StringComparison.OrdinalIgnoreCase) ||
             clip.StartsWith("magnet:", StringComparison.OrdinalIgnoreCase)))
        {
            Url = clip;
        }
    }

    private void TryInferFilename(string urlString)
    {
        try
        {
            if (Uri.TryCreate(urlString, UriKind.Absolute, out Uri? uri))
            {
                string leaf = Path.GetFileName(uri.LocalPath);
                if (!string.IsNullOrWhiteSpace(leaf) && leaf.Contains('.'))
                {
                    Filename = leaf;
                }
            }
        }
        catch
        {
            // ignore URI parse issues
        }
    }

    private async Task ExecuteAddAsync()
    {
        string cleanUrl = Url.Trim();
        if (string.IsNullOrWhiteSpace(cleanUrl))
        {
            ErrorMessage = "Please enter a valid URL.";
            return;
        }

        if (!cleanUrl.StartsWith("http://", StringComparison.OrdinalIgnoreCase) &&
            !cleanUrl.StartsWith("https://", StringComparison.OrdinalIgnoreCase) &&
            !cleanUrl.StartsWith("magnet:", StringComparison.OrdinalIgnoreCase))
        {
            ErrorMessage = "Only HTTP(S) and magnet: URLs are supported.";
            return;
        }

        IsSubmitting = true;
        ErrorMessage = null;

        try
        {
            var result = await _daemonClient.AddDownloadAsync(cleanUrl, string.IsNullOrWhiteSpace(Filename) ? null : Filename.Trim());
            if (result != null)
            {
                RequestClose?.Invoke(true);
            }
            else
            {
                ErrorMessage = "Failed to add download. Please verify daemon connection and token.";
            }
        }
        catch (Exception ex)
        {
            ErrorMessage = $"Error adding download: {ex.Message}";
        }
        finally
        {
            IsSubmitting = false;
        }
    }
}
