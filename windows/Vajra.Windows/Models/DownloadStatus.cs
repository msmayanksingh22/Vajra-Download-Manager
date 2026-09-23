namespace Vajra.Windows.Models;

public enum ConnectionState
{
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Unauthorized
}

public enum DownloadStatus
{
    Downloading,
    Connecting,
    Completed,
    Paused,
    Failed,
    Idle,
    Verifying
}
