using Vajra.Windows.Models;

namespace Vajra.Windows.Services;

public interface ISseService : IDisposable
{
    event Action<SseProgressPayload>? OnProgress;
    event Action<SseBatchProgressPayload>? OnBatchProgress;
    event Action<SseStateChangePayload>? OnStateChange;
    event Action<SseAddedPayload>? OnAdded;
    event Action<SseRemovedPayload>? OnRemoved;
    event Action? OnConnected;
    event Action? OnDisconnected;
    event Action? OnAuthenticationFailed;

    bool IsConnected { get; }
    Task StartAsync(CancellationToken ct = default);
    Task StopAsync();
}
