//! SSE broadcaster for real-time download events.
//!
//! All axum handlers push events into `SseBroadcaster`. The global SSE
//! endpoint (`GET /api/v1/events`) fans them out to all connected clients.
//! Per-download SSE (`GET /api/v1/downloads/:id/events`) filters by ID.

use std::sync::Arc;

use axum::response::sse::Event;
use tokio::sync::broadcast;
use vajra_protocol::DaemonEvent;

/// How many events the broadcast channel buffers before dropping slow clients.
const CHANNEL_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct SseBroadcaster {
    tx: Arc<broadcast::Sender<Arc<DaemonEvent>>>,
}

impl SseBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx: Arc::new(tx) }
    }

    /// Publish an event to all active SSE subscribers.
    pub fn send(&self, event: DaemonEvent) {
        // Ignore error — no subscribers is fine
        let _ = self.tx.send(Arc::new(event));
    }

    /// Subscribe to the broadcast stream.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<DaemonEvent>> {
        self.tx.subscribe()
    }
}

/// Serialize a `DaemonEvent` into an SSE `Event` for axum.
pub fn to_sse_event(ev: &DaemonEvent) -> Result<Event, serde_json::Error> {
    let (event_name, data) = match ev {
        DaemonEvent::Progress { .. } => ("progress", serde_json::to_string(ev)?),
        DaemonEvent::StateChange { .. } => ("state_change", serde_json::to_string(ev)?),
        DaemonEvent::HashResult { .. } => ("hash_result", serde_json::to_string(ev)?),
        DaemonEvent::Added { .. } => ("added", serde_json::to_string(ev)?),
        DaemonEvent::Removed { .. } => ("removed", serde_json::to_string(ev)?),
        DaemonEvent::Intercepted { .. } => ("intercepted", serde_json::to_string(ev)?),
        DaemonEvent::BatchProgress { .. } => ("batch_progress", serde_json::to_string(ev)?),
    };
    Ok(Event::default().event(event_name).data(data))
}
