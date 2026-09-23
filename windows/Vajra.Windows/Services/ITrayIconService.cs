namespace Vajra.Windows.Services;

public interface ITrayIconService : IDisposable
{
    event Action? OnOpenRequested;
    event Action? OnAddDownloadRequested;
    event Action? OnPauseAllRequested;
    event Action? OnResumeAllRequested;
    event Action? OnExitRequested;

    void Initialize();
    void UpdateStatus(int activeCount, ulong speedBps);
    void ShowNotification(string title, string message);
}
