//! Download Queue Manager
//!
//! Manages a priority queue of `DownloadTask`s with configurable concurrency.
//! The Tauri backend holds one `DownloadManagerHandle` and drives everything
//! through it.

use std::sync::{Arc, Weak};

use serde::{Deserialize, Serialize};
use indexmap::IndexMap;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::download_task::{DownloadProgress, DownloadRequest, DownloadTask, TaskId, TaskState};

/// Settings snapshot for the queue manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueSettings {
    /// Maximum simultaneous active downloads.
    pub max_concurrent: usize,
    /// Is the global time scheduler enabled?
    pub scheduler_enabled: bool,
    /// Time to start queue (e.g. "02:00")
    pub scheduler_start_time: Option<String>,
    /// Time to stop queue (e.g. "06:00")
    pub scheduler_stop_time: Option<String>,

    // --- Fair Access Policy (Quotas) ---
    pub fap_enabled: bool,
    pub fap_quota_bytes: u64,
    pub fap_time_window_secs: u64,
}

impl Default for QueueSettings {
    fn default() -> Self {
        Self {
            max_concurrent: 3,
            scheduler_enabled: false,
            scheduler_start_time: None,
            scheduler_stop_time: None,
            fap_enabled: false,
            fap_quota_bytes: 10 * 1024 * 1024, // 10MB default
            fap_time_window_secs: 3600,        // 1 hour default
        }
    }
}

/// A queue entry wrapping an active or pending task.
#[derive(Clone)]
pub struct QueueEntry {
    pub id: TaskId,
    pub request: DownloadRequest,
    pub task: Option<DownloadTask>, // None = queued but not yet started
    pub last_sync_unix: Option<i64>,
    /// When this entry first reached a terminal state (Completed/Failed/Cancelled).
    /// Used by tick() to prune settled entries after a short grace period so the
    /// SSE consumer has time to emit the final event before the entry disappears.
    pub terminal_since: Option<std::time::Instant>,
}

/// Thread-safe download manager shared between Tauri commands.
#[derive(Clone)]
pub struct DownloadManagerHandle(Arc<DownloadManager>);

impl std::ops::Deref for DownloadManagerHandle {
    type Target = DownloadManager;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct FapState {
    pub window_start_unix: i64,
    pub bytes_in_window: u64,
    pub active_previous_bytes: std::collections::HashMap<TaskId, u64>,
}

pub struct DownloadManager {
    /// All entries: active + queued (in order).
    entries: RwLock<IndexMap<TaskId, QueueEntry>>,
    /// Settings (concurrency limit etc.)
    settings: Mutex<QueueSettings>,
    /// Global throttle shared across all active download tasks.
    global_throttle: crate::throttle::Throttle,
    /// Fair Access Policy state tracking.
    fap_state: Mutex<FapState>,
    /// Rules engine for automation
    rules_engine: Arc<crate::rules::RulesEngine>,
}

impl DownloadManager {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(settings: QueueSettings, global_speed_limit_bps: u64) -> DownloadManagerHandle {
        let manager = Arc::new(Self {
            entries: RwLock::new(IndexMap::new()),
            settings: Mutex::new(settings),
            global_throttle: crate::throttle::Throttle::new(global_speed_limit_bps),
            fap_state: Mutex::new(FapState {
                window_start_unix: chrono::Local::now().timestamp(),
                bytes_in_window: 0,
                active_previous_bytes: std::collections::HashMap::new(),
            }),
            rules_engine: Arc::new(crate::rules::RulesEngine::default()),
        });
        Self::start_scheduler(Arc::downgrade(&manager));
        DownloadManagerHandle(manager)
    }

    /// Update the global speed limit at runtime.
    pub async fn set_global_limit(&self, limit_bps: u64) {
        self.global_throttle.set_limit(limit_bps).await;
    }

    /// Add a download to the queue and start it if concurrency allows.
    pub async fn add(&self, request: DownloadRequest) -> TaskId {
        let id = Uuid::new_v4();
        self.add_with_id(id, request).await;
        id
    }

    /// Restore or enqueue a request with a durable identifier.
    pub async fn add_with_id(&self, id: TaskId, mut request: DownloadRequest) {
        self.rules_engine.evaluate_and_apply(&mut request);
        let entry = QueueEntry {
            id,
            request: request.clone(),
            task: None,
            last_sync_unix: None,
            terminal_since: None,
        };

        {
            let mut entries = self.entries.write().await;
            entries.insert(id, entry);
        }

        self.tick().await;
    }

    /// Restore a download task directly to the manager.
    pub async fn add_restored(&self, id: TaskId, request: DownloadRequest, task: DownloadTask) {
        let entry = QueueEntry {
            id,
            request,
            task: Some(task),
            last_sync_unix: None,
            terminal_since: None,
        };

        let mut entries = self.entries.write().await;
        entries.insert(id, entry);
    }

    /// Update the URL of a download, allowing resuming with the new URL.
    pub async fn update_url(&self, id: TaskId, new_url: String) -> Result<(), String> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&id) {
            // Stop any active task
            if let Some(task) = &entry.task {
                task.cancel().await;
            }
            entry.task = None;

            // Update sidecar state file if it exists
            let filename = entry.request.filename.clone().unwrap_or_else(|| {
                entry
                    .request
                    .url
                    .split('?')
                    .next()
                    .unwrap_or(&entry.request.url)
                    .split('/')
                    .next_back()
                    .unwrap_or("download")
                    .to_string()
            });
            let state_path = entry
                .request
                .dest_dir
                .join(format!(".{}.vajra.state", filename));

            if state_path.exists() {
                if let Ok(Some(mut state)) = crate::state::DownloadState::load(&state_path) {
                    state.url = new_url.clone();
                    let _ = state.save(&state_path);
                }
            }

            // Update the URL in request
            entry.request.url = new_url;
            Ok(())
        } else {
            Err("Download not found".to_string())
        }
    }

    pub async fn update_tags(&self, id: TaskId, tags: Vec<String>) -> Result<(), String> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&id) {
            entry.request.tags = tags;
            // The running task (if any) won't instantly emit progress with new tags 
            // since it holds a clone of the request, but on next resume it will.
            // That's fine for tags.
        } else {
            return Err("Task not found".to_string());
        }
        Ok(())
    }

    /// Update the filename of a download, renaming files on disk if they exist.
    pub async fn update_filename(&self, id: TaskId, new_filename: String) -> Result<(), String> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&id) {
            // Get current progress/state of the running/completed task before cancelling
            let old_task_state = if let Some(task) = &entry.task {
                let prog = task.progress();
                let state = prog.state.clone();
                task.cancel().await;
                Some((state, prog.bytes_downloaded, prog.total_bytes, prog.error.clone()))
            } else {
                None
            };
            entry.task = None;

            // Old filename
            let old_filename = entry.request.filename.clone().unwrap_or_else(|| {
                let base = entry.request.url.split('#').next().unwrap_or(&entry.request.url);
                let base = base.split('?').next().unwrap_or(base);
                base.split('/')
                    .next_back()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("download")
                    .to_string()
            });

            let mut final_dest_path = entry.request.dest_dir.join(&old_filename);

            if old_filename != new_filename {
                let dest_dir = &entry.request.dest_dir;
                let old_dest_path = dest_dir.join(&old_filename);
                let new_dest_path = dest_dir.join(&new_filename);
                final_dest_path = new_dest_path.clone();

                // Rename main download file (e.g. video.mp4)
                if old_dest_path.exists() {
                    let _ = std::fs::rename(&old_dest_path, &new_dest_path);
                }

                // Also rename the sidecar state file if it exists
                let old_state_path = dest_dir.join(format!(".{}.vajra.state", old_filename));
                let new_state_path = dest_dir.join(format!(".{}.vajra.state", new_filename));

                if old_state_path.exists() {
                    let _ = std::fs::rename(&old_state_path, &new_state_path);
                }
            }

            // Update the filename in request
            entry.request.filename = Some(new_filename.clone());

            // If the old task was in a terminal state, restore it in the same state so it doesn't auto-start
            if let Some((state, bytes_downloaded, total_bytes, error)) = old_task_state {
                if matches!(
                    state,
                    TaskState::Paused | TaskState::Failed | TaskState::Completed
                ) {
                    let restored = DownloadTask::new_restored(
                        entry.id,
                        entry.request.clone(),
                        state,
                        bytes_downloaded,
                        total_bytes,
                        new_filename,
                        final_dest_path.to_string_lossy().to_string(),
                        error,
                    );
                    entry.task = Some(restored);
                }
            }

            Ok(())
        } else {
            Err("Download not found".to_string())
        }
    }

    /// Update settings of a download task (speed limit, max connections).
    pub async fn update_download_settings(
        &self,
        id: TaskId,
        speed_limit: Option<u64>,
        max_connections: Option<u32>,
    ) -> Result<(), String> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(&id) {
            if let Some(limit) = speed_limit {
                entry.request.speed_limit = limit;
                if let Some(ref task) = entry.task {
                    if let Some(ref throttle) = task.request.throttle {
                        let th = throttle.clone();
                        tokio::spawn(async move {
                            th.local.set_limit(limit).await;
                        });
                    }
                }
            }
            if let Some(connections) = max_connections {
                entry.request.max_connections = connections;
            }
            Ok(())
        } else {
            Err("Download not found".to_string())
        }
    }

    /// Pause a specific download by ID.
    pub async fn pause(&self, id: TaskId) {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(&id) {
            if let Some(task) = &entry.task {
                task.pause().await;
            }
        }
    }

    /// Resume a paused download.
    ///
    /// Sends the pause signal to any in-flight task first, then waits for it to
    /// reach a settled state (`Paused`, `Failed`, or `Cancelled`) before clearing
    /// `entry.task`.  This prevents `tick()` from creating a second `DownloadTask`
    /// while the original task is still active — which would cause two writers to
    /// race on the same pre-allocated file.
    pub async fn resume(&self, id: TaskId) {
        // Step 1: retrieve and pause the current task (if any) under a read-lock.
        let maybe_task = {
            let entries = self.entries.read().await;
            entries
                .get(&id)
                .and_then(|e| e.task.clone())
        };

        if let Some(task) = maybe_task {
            // Send the pause signal; this is fast (just sends on a channel).
            task.pause().await;

            // Wait for the task to actually stop. Poll with a short sleep so we
            // don't busy-spin, but cap at 3 seconds to avoid blocking forever.
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                let state = task.progress().state;
                if matches!(
                    state,
                    TaskState::Paused
                        | TaskState::Pausing
                        | TaskState::Failed
                        | TaskState::Cancelled
                        | TaskState::Completed
                ) {
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    // Timeout: proceed anyway; the task will either finish soon
                    // or its `AbortHandle` will be dropped when the old entry is
                    // replaced, causing the underlying future to be cancelled.
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }

        // Step 2: clear the task slot and let tick() schedule a fresh start.
        {
            let mut entries = self.entries.write().await;
            if let Some(entry) = entries.get_mut(&id) {
                entry.task = None;
            }
        }
        self.tick().await;
    }

    /// Pause all active downloads.
    pub async fn pause_all(&self) {
        let entries = self.entries.read().await;
        for entry in entries.values() {
            if let Some(task) = &entry.task {
                task.pause().await;
            }
        }
    }

    /// Resume all paused or failed downloads.
    pub async fn resume_all(&self) {
        {
            let mut entries = self.entries.write().await;
            for entry in entries.values_mut() {
                if let Some(task) = &entry.task {
                    let state = task.progress().state;
                    if state == TaskState::Paused || state == TaskState::Failed {
                        entry.task = None;
                    }
                } else {
                    // Also covers entries that never started
                    entry.task = None;
                }
            }
        }
        self.tick().await;
    }

    /// Cancel and remove a download.
    ///
    /// Both the cancel signal and the Vec removal happen under a single write-lock
    /// to prevent a TOCTOU race where tick() inserts a new task between the two
    /// operations.
    pub async fn cancel(&self, id: TaskId) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get(&id) {
            if let Some(task) = &entry.task {
                task.cancel().await;
            }
        }
        entries.shift_remove(&id);
    }

    /// Get a progress snapshot for all entries.
    pub async fn all_progress(&self) -> Vec<DownloadProgress> {
        let entries = self.entries.read().await;
        entries
            .values()
            .map(|entry| {
                let mut p = entry
                    .task
                    .as_ref()
                    .map(|task| task.progress())
                    .unwrap_or_else(|| get_entry_progress(entry));
                p.speed_limit_bps = entry.request.speed_limit;
                p
            })
            .collect()
    }

    /// Get progress for a single download.
    pub async fn progress(&self, id: TaskId) -> Option<DownloadProgress> {
        let entries = self.entries.read().await;
        entries.get(&id).map(|entry| {
            let mut p = entry
                .task
                .as_ref()
                .map(|task| task.progress())
                .unwrap_or_else(|| get_entry_progress(entry));
            p.speed_limit_bps = entry.request.speed_limit;
            p
        })
    }

    /// Update settings.
    pub async fn set_settings(&self, s: QueueSettings) {
        let mut settings = self.settings.lock().await;
        *settings = s;
        drop(settings);
        self.tick().await;
    }

    /// Internal: start as many queued downloads as concurrency allows.
    ///
    /// Structured in three distinct phases to avoid lock-ordering starvation:
    ///
    /// 1. **Settings snapshot** – read `QueueSettings` and compute scheduler allowance
    ///    (no entries lock).
    /// 2. **FAP accounting** – take a short read-lock snapshot of byte counts, then
    ///    update `fap_state` under its own mutex.  Both locks are fully released before
    ///    phase 3 begins.
    /// 3. **Task management** – acquire the entries write-lock *once* for the minimal
    ///    window: pause active tasks or start queued ones.
    ///
    /// Previously, phase 2 ran *inside* the entries write-lock, blocking every
    /// concurrent `all_progress()` read for the full 250 ms scheduler interval.
    async fn tick(&self) {
        let settings = self.settings.lock().await;
        let max_concurrent = settings.max_concurrent;
        let scheduler_enabled = settings.scheduler_enabled;
        let start_time_str = settings.scheduler_start_time.clone();
        let stop_time_str = settings.scheduler_stop_time.clone();
        let fap_enabled = settings.fap_enabled;
        let fap_quota_bytes = settings.fap_quota_bytes;
        let fap_time_window_secs = settings.fap_time_window_secs;
        drop(settings);

        // ── Phase 1: scheduler allowance (no locks) ───────────────────────────
        let mut global_time_allowed = true;
        let now = chrono::Local::now();
        let now_unix = now.timestamp();

        if scheduler_enabled {
            if let (Some(start_str), Some(stop_str)) = (&start_time_str, &stop_time_str) {
                if let (Ok(start), Ok(stop)) = (
                    chrono::NaiveTime::parse_from_str(start_str, "%H:%M"),
                    chrono::NaiveTime::parse_from_str(stop_str, "%H:%M"),
                ) {
                    let current = now.time();
                    if start < stop {
                        if current < start || current >= stop {
                            global_time_allowed = false;
                        }
                    } else {
                        // crosses midnight
                        if current < start && current >= stop {
                            global_time_allowed = false;
                        }
                    }
                }
            }
        }

        // ── Phase 2: FAP accounting (short read-lock, then fap_state mutex) ──
        let fap_allowed = if fap_enabled {
            // 2a. Snapshot per-task byte counts under a short read-lock only.
            let byte_snapshot: Vec<(TaskId, u64)> = {
                let entries = self.entries.read().await;
                entries
                    .values()
                    .filter_map(|e| {
                        e.task
                            .as_ref()
                            .map(|t| (e.id, t.progress().bytes_downloaded))
                    })
                    .collect()
            }; // read-lock dropped here

            // 2b. Update FAP state under its own dedicated mutex.
            //     No entries lock is held at this point.
            let mut fap = self.fap_state.lock().await;
            if now_unix - fap.window_start_unix >= fap_time_window_secs as i64 {
                fap.window_start_unix = now_unix;
                fap.bytes_in_window = 0;
                fap.active_previous_bytes.clear();
            }
            let active_ids: std::collections::HashSet<TaskId> =
                byte_snapshot.iter().map(|(id, _)| *id).collect();
            for (id, current_bytes) in &byte_snapshot {
                let prev = *fap.active_previous_bytes.get(id).unwrap_or(current_bytes);
                if *current_bytes > prev {
                    fap.bytes_in_window = fap.bytes_in_window.saturating_add(current_bytes - prev);
                }
                fap.active_previous_bytes.insert(*id, *current_bytes);
            }
            // Remove stale entries for tasks no longer in the queue.
            fap.active_previous_bytes
                .retain(|id, _| active_ids.contains(id));
            let allowed = fap.bytes_in_window < fap_quota_bytes;
            drop(fap); // release fap_state mutex before taking write-lock
            allowed
        } else {
            true
        };

        // ── Phase 3: task management (entries write-lock, minimal window) ─────
        let mut entries = self.entries.write().await;

        // Count currently active downloads.
        let active_count = entries
            .values()
            .filter(|e| {
                e.task
                    .as_ref()
                    .map(|t| {
                        matches!(
                            t.progress().state,
                            TaskState::Downloading
                                | TaskState::Allocating
                                | TaskState::FetchingMeta
                        )
                    })
                    .unwrap_or(false)
            })
            .count();

        // Pause all active tasks when the scheduler window or FAP disallows traffic.
        if !global_time_allowed || !fap_allowed {
            if active_count > 0 {
                for entry in entries.values() {
                    if let Some(task) = &entry.task {
                        let t = task.clone();
                        tokio::spawn(async move {
                            t.pause().await;
                        });
                    }
                }
            }
            return;
        }

        // Start queued downloads up to the concurrency limit.
        let slots = max_concurrent.saturating_sub(active_count);

        if slots > 0 {
            let mut queued_keys: Vec<TaskId> = entries
                .values()
                .filter(|e| {
                    if e.task.is_some() {
                        return false;
                    }
                    // Respect individual per-job schedule.
                    if let Some(scheduled_for) = e.request.schedule_at {
                        if now_unix < scheduled_for {
                            return false;
                        }
                    }
                    true
                })
                .map(|e| e.id)
                .collect();

            // Sort indices by Priority (High < Normal < Low). Stable sort preserves insertion order.
            queued_keys.sort_by_key(|id| entries.get(id).map(|e| e.request.priority.clone()).unwrap_or(vajra_protocol::Priority::Low));

            for id in queued_keys.into_iter().take(slots) {
                if let Some(entry) = entries.get_mut(&id) {
                    let mut req = entry.request.clone();
                    req.throttle = Some(crate::throttle::CombinedThrottle::new(
                        self.global_throttle.clone(),
                        req.speed_limit,
                    ));
                    let task = DownloadTask::start_with_id(entry.id, req);
                    entry.task = Some(task);
                }
            }
        }

        // Pruning of terminal entries removed.
        // Completed/Failed/Cancelled tasks now remain in the manager's memory
        // so they continue to be visible in the UI until explicitly cleared or restarted.
        for entry in entries.values_mut() {
            if let Some(task) = &entry.task {
                let state = task.progress().state;
                if matches!(
                    state,
                    TaskState::Completed | TaskState::Failed | TaskState::Cancelled
                ) {
                    entry.terminal_since.get_or_insert(std::time::Instant::now());
                } else {
                    entry.terminal_since = None;
                }
            }
        }

    }

    fn start_scheduler(manager: Weak<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(250));
            loop {
                interval.tick().await;
                let Some(manager) = manager.upgrade() else {
                    break;
                };
                manager.tick().await;
            }
        });
    }

    /// Export the current queue to JSON for sharing.
    pub async fn export_queue_json(&self) -> Result<String, String> {
        let entries = self.entries.read().await;
        let requests: Vec<DownloadRequest> = entries.values().map(|e| e.request.clone()).collect();
        serde_json::to_string_pretty(&requests).map_err(|e| e.to_string())
    }

    /// Import a queue from JSON.
    pub async fn import_queue_json(&self, json_data: &str) -> Result<usize, String> {
        let requests: Vec<DownloadRequest> = serde_json::from_str(json_data).map_err(|e| e.to_string())?;
        let count = requests.len();
        for req in requests {
            self.add(req).await;
        }
        Ok(count)
    }
}

fn get_entry_progress(entry: &QueueEntry) -> DownloadProgress {
    let filename = entry.request.filename.clone().unwrap_or_else(|| {
        let base = entry.request.url.split('#').next().unwrap_or(&entry.request.url);
        let base = base.split('?').next().unwrap_or(base);
        base.split('/')
            .next_back()
            .filter(|s| !s.is_empty())
            .unwrap_or("download")
            .to_string()
    });
    let state_path = entry
        .request
        .dest_dir
        .join(format!(".{}.vajra.state", filename));

    let mut bytes_downloaded = 0;
    let mut total_bytes = 0;
    let mut progress_fraction = 0.0;

    if let Ok(Some(saved)) = crate::state::DownloadState::load(&state_path) {
        total_bytes = saved.total_bytes;
        bytes_downloaded = saved.chunks.iter().map(|c| c.bytes_written).sum();
        if total_bytes > 0 {
            progress_fraction = bytes_downloaded as f64 / total_bytes as f64;
        }
    } else {
        let path = entry.request.dest_dir.join(&filename);
        if let Ok(metadata) = std::fs::metadata(&path) {
            bytes_downloaded = metadata.len();
            total_bytes = metadata.len();
            progress_fraction = 1.0;
        }
    }

    DownloadProgress {
        id: entry.id,
        url: entry.request.url.clone(),
        state: TaskState::Queued,
        bytes_downloaded,
        total_bytes,
        speed_bps: 0,
        eta_secs: 0,
        progress_fraction,
        chunk_fractions: Vec::new(),
        filename,
        dest_path: entry.request.dest_dir.to_string_lossy().into_owned(),
        error: None,
        segments: Vec::new(),
        resume_supported: true,
        hash_result: None,
        expected_hash: entry.request.expected_hash.clone(),
        queue_type: entry.request.queue_type.clone(),
        sync_interval_secs: entry.request.sync_interval_secs,
        tags: entry.request.tags.clone(),
        speed_limit_bps: entry.request.speed_limit,
    }
}
