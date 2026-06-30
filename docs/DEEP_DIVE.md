# Vajra Download Manager — Deep Dive Implementation Guide

> **Companion to:** `COMPREHENSIVE_REVIEW.md`
> **Date:** 2026-06-24
> **Purpose:** Concrete implementation guides, code patches, and architectural proposals
> **Focus:** Actionable fixes, feature implementations, and migration paths

---

## Table of Contents

1. [Critical Bug Fixes with Code Patches (Completed)](#1-critical-bug-fixes-with-code-patches-completed)
2. [Security Hardening Implementation (Completed)](#2-security-hardening-implementation-completed)
3. [Feature Implementation Proposals (Completed)](#3-feature-implementation-proposals-completed)
4. [Architecture Evolution Proposals (Completed)](#4-architecture-evolution-proposals-completed)
5. [Performance Optimization Deep-Dive (Completed)](#5-performance-optimization-deep-dive-completed)
6. [Testing Strategy with Examples (Completed)](#6-testing-strategy-with-examples-completed)
7. [Frontend Refactoring Blueprint (Completed)](#7-frontend-refactoring-blueprint-completed)
8. [Distribution & Deployment Plan](#8-distribution--deployment-plan)
9. [UX/UI Enhancement Proposals (Completed)](#9-uxui-enhancement-proposals-completed)
10. [Competitive Feature Parity Guide (Completed)](#10-competitive-feature-parity-guide-completed)
11. [Migration Paths](#11-migration-paths)
12. [Performance Benchmarks & Targets](#12-performance-benchmarks--targets)

---

## 1. Critical Bug Fixes with Code Patches (Completed)

### 1.1 Fix Double Post-Processing

**Problem:** Post-processing runs twice for HTTP downloads (once in `run_download`, once in `download_inner`).

**Current Code (`download_task.rs:307-386` and `937-997`):**
```rust
// FIRST OCCURRENCE (lines 307-386) - in run_download()
let result = download_inner(&tx, req, progress, ctrl_rx).await;

// Hash verification
if let Some(expected) = &req.expected_hash {
    match verify_hash(&dest_path, expected).await {
        Ok(hash_result) => {
            p.hash_result = Some(hash_result.clone());
            if !hash_result.matched {
                p.status = TaskState::Failed;
                p.error = Some("Hash mismatch".to_string());
            }
        }
        Err(e) => {
            tracing::warn!("Hash verification error: {}", e);
        }
    }
}

// AV scan
if let Some(av_path) = &av_scan_path {
    match run_antivirus_scan(&dest_path, av_path, &av_args).await {
        Ok(true) => {}, // clean
        Ok(false) => {
            p.status = TaskState::Failed;
            p.error = Some("Virus detected".to_string());
        }
        Err(e) => {
            tracing::warn!("AV scan error: {}", e);
        }
    }
}

// ... auto-extract and post-processing script ...

// SECOND OCCURRENCE (lines 937-997) - in download_inner()
// SAME CODE REPEATED!

Ok(bytes)
```

**Fix:** Remove the post-processing from `download_inner()` and keep only in `run_download()`.

```rust
// download_task.rs - download_inner() should return raw bytes
// Remove lines 937-997 entirely

// At the end of download_inner():
Ok(bytes)

// run_download() already has the post-processing pipeline at lines 307-386
// This is the correct place since it covers all protocol paths
```

**Files to change:** `vajra-engine/src/download_task.rs`
**Lines to delete:** 937-997 (the second post-processing block in `download_inner`)

### 1.2 Fix `.truncate(true)` Destroying Resume Data

**Problem:** Linux and macOS `preallocate` use `.truncate(true)`, destroying partial downloads on retry.

**Current Code (`allocator.rs:75` Linux, `245` macOS):**
```rust
let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .truncate(true)  // ← DESTRUCTIVE: deletes all existing data!
    .open(path)?;
```

**Fix:** Match Windows behavior - only truncate if file doesn't exist or is smaller than target.

```rust
// allocator.rs - Linux implementation (around line 65)
pub fn preallocate_file(path: &Path, size: u64) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    
    // Check if file exists and get current size
    let existing_size = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(0);
    
    // Only open with truncate if we need to grow the file
    let should_truncate = size > existing_size;
    
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(should_truncate)
        .open(path)?;
    
    if size == 0 {
        return Ok(());
    }
    
    // Use fallocate for efficient allocation
    let fd = file.as_raw_fd();
    let ret = unsafe {
        libc::fallocate(fd, 0, 0, size as i64)
    };
    
    if ret == -1 {
        let err = io::Error::last_os_error();
        // Fallback if fallocate not supported (e.g., some filesystems)
        if err.raw_os_error() == Some(libc::EOPNOTSUPP) {
            file.set_len(size)?;
        } else {
            return Err(err);
        }
    }
    
    Ok(())
}

// Same fix for macOS implementation (around line 240)
// Use F_PREALLOCATE with existing size check
```

**Files to change:** `vajra-engine/src/allocator.rs`
**Impact:** Prevents data loss on download retry/resume

### 1.3 Fix Work-Stealing TOCTOU Race

**Problem:** Race condition in `steal_from_slowest` can cause duplicate writes.

**Current Code (`multiplexer.rs:216-254`):**
```rust
async fn steal_from_slowest(
    shared: &Arc<Mutex<Vec<Chunk>>>,
    worker_id: usize,
) -> Option<Chunk> {
    let mut guard = shared.lock().await;
    
    // Find chunk with most remaining bytes
    let donor_idx = guard.iter()
        .enumerate()
        .filter(|(_, c)| c.status == ChunkStatus::Downloading)
        .max_by_key(|(_, c)| c.remaining_bytes())?;
    
    // READ current_offset
    let donor = &mut guard[donor_idx];
    let midpoint = donor.current_offset + donor.remaining_bytes() / 2;
    
    // MODIFY donor's end_byte
    donor.end_byte = midpoint - 1;
    
    // CREATE new chunk from midpoint
    let new_chunk = Chunk {
        start_byte: midpoint,
        end_byte: donor.original_end_byte,
        // ...
    };
    
    Some(new_chunk)
}
```

**Problem:** Between reading `current_offset` and modifying `end_byte`, the donor task may have advanced `current_offset`. This creates an overlap.

**Fix 1: Atomic Offset Update (Recommended)**

Make the steal operation atomic by having the donor participate:

```rust
// multiplexer.rs - Add steal request mechanism

pub struct Chunk {
    pub id: usize,
    pub start_byte: u64,
    pub end_byte: u64,
    pub current_offset: u64,
    pub status: ChunkStatus,
    // Add:
    pub steal_request: Option<StealRequest>,
}

pub struct StealRequest {
    pub worker_id: usize,
    pub split_at: u64,
    pub response_tx: oneshot::Sender<StealResponse>,
}

pub struct StealResponse {
    pub approved: bool,
    pub actual_split_at: u64,
}

// In worker loop (run_chunk):
loop {
    // Check for steal requests between chunks
    if let Some(request) = chunk.steal_request.take() {
        let current_pos = chunk.start_byte + bytes_emitted;
        if current_pos < request.split_at {
            // Approve the split
            chunk.end_byte = current_pos - 1;
            let _ = request.response_tx.send(StealResponse {
                approved: true,
                actual_split_at: current_pos,
            });
            break; // Exit current chunk loop
        } else {
            // Reject - already past the split point
            let _ = request.response_tx.send(StealResponse {
                approved: false,
                actual_split_at: current_pos,
            });
        }
    }
    
    // Continue downloading...
}
```

**Fix 2: Writer-Side Overlap Detection (Defensive)**

Add overlap detection in the writer as a safety net:

```rust
// writer.rs - Add overlap tracking

use std::collections::BTreeMap;

pub struct WriteTracker {
    written_ranges: BTreeMap<u64, u64>, // offset -> length
}

impl WriteTracker {
    pub fn check_overlap(&self, offset: u64, len: u64) -> Result<(), io::Error> {
        let end = offset + len as u64;
        
        // Check for overlap with any existing range
        for (&existing_offset, &existing_len) in &self.written_ranges {
            let existing_end = existing_offset + existing_len;
            
            // Check if ranges overlap
            if offset < existing_end && end > existing_offset {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Write overlap detected: [{},{}) overlaps with [{},{})",
                        offset, end, existing_offset, existing_end
                    )
                ));
            }
        }
        
        Ok(())
    }
    
    pub fn record_write(&mut self, offset: u64, len: u64) {
        self.written_ranges.insert(offset, len);
    }
}

// In start_disk_writer:
let mut tracker = WriteTracker::new();

for frame in rx {
    // Check for overlap before writing
    tracker.check_overlap(frame.absolute_offset, frame.payload.len() as u64)?;
    
    // Write the data
    write_all_at(&file, &frame.payload, frame.absolute_offset)?;
    
    // Record the write
    tracker.record_write(frame.absolute_offset, frame.payload.len() as u64);
}
```

**Files to change:** 
- `vajra-engine/src/multiplexer.rs` (steal logic)
- `vajra-engine/src/writer.rs` (overlap detection)

### 1.4 Fix `unwrap()` Panic in HLS

**Problem:** `unwrap()` on file creation panics if temp directory is deleted or disk is full.

**Current Code (`hls.rs:112`):**
```rust
let mut file = std::fs::File::create(&ts_path).unwrap();
```

**Fix:**
```rust
// hls.rs - Replace unwrap with proper error handling
let mut file = std::fs::File::create(&ts_path).map_err(|e| {
    tracing::error!("Failed to create temp file {:?}: {}", ts_path, e);
    e
})?;

// Also fix silent error swallowing in the same block (lines 110-116)
let resp = client_clone.get(&seg.url).send().await;
match resp {
    Ok(mut response) => {
        while let Ok(Some(chunk)) = response.chunk().await {
            if let Err(e) = file.write_all(&chunk) {
                tracing::error!("Failed to write segment {}: {}", seg.url, e);
                segment_errors.push((seg.index, e));
                break;
            }
        }
    }
    Err(e) => {
        tracing::error!("Failed to download segment {}: {}", seg.url, e);
        segment_errors.push((seg.index, e));
    }
}

// After all segments complete, check for errors
if !segment_errors.is_empty() {
    return Err(anyhow::anyhow!(
        "{} segments failed to download: {:?}",
        segment_errors.len(),
        segment_errors.iter().map(|(idx, _)| idx).collect::<Vec<_>>()
    ));
}
```

**Files to change:** `vajra-engine/src/hls.rs`

### 1.5 Fix Database Lock Scope

**Problem:** Database mutex held during SSE broadcast, blocking API endpoints.

**Current Code (`main.rs:325-426`):**
```rust
async fn progress_loop(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_millis(150));
    
    loop {
        interval.tick().await;
        
        let db = state.database.lock().await;  // ← Lock acquired here
        
        let all = state.manager.all_progress().await;
        
        for entry in &all {
            // ... update database ...
            db.update_job_state(&entry.id, &entry.status).await?;
            // ... more DB operations ...
        }
        
        // SSE broadcast happens here while db lock is still held!
        for entry in &all {
            let event = DaemonEvent::Progress { ... };
            let _ = state.sse_tx.send(Arc::new(event));
        }
        
        // Lock released at end of scope
    }
}
```

**Fix:**
```rust
async fn progress_loop(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_millis(150));
    
    loop {
        interval.tick().await;
        
        // Phase 1: Database updates (scoped lock)
        {
            let db = state.database.lock().await;
            let all = state.manager.all_progress().await;
            
            for entry in &all {
                if entry.status.is_terminal() {
                    if let Err(e) = db.insert_history(&entry).await {
                        tracing::warn!("Failed to insert history: {}", e);
                    }
                } else {
                    if let Err(e) = db.update_job_state(&entry.id, &entry.status).await {
                        tracing::warn!("Failed to update job state: {}", e);
                    }
                }
            }
        } // ← Lock released here
        
        // Phase 2: SSE broadcast (no lock held)
        let all = state.manager.all_progress().await;
        for entry in &all {
            let event = DaemonEvent::Progress {
                id: entry.id,
                bytes_done: entry.bytes_done,
                speed_bps: entry.speed_bps,
                eta_seconds: entry.eta_seconds,
                segments: entry.segments.clone(),
            };
            let _ = state.sse_tx.send(Arc::new(event));
        }
    }
}
```

**Files to change:** `vajra-daemon/src/main.rs`

### 1.6 Replace `std::process::exit(0)` with Graceful Shutdown

**Problem:** `std::process::exit(0)` skips destructors, doesn't flush I/O, leaves corrupted files.

**Current Code (`main.rs:263`):**
```rust
fn execute_post_queue_action(action: PostQueueAction) {
    match action {
        PostQueueAction::ExitApp => {
            std::process::exit(0);
        }
        // ...
    }
}
```

**Fix:**
```rust
// main.rs - Use a shutdown channel

use tokio::sync::broadcast;

struct AppState {
    // ... existing fields ...
    shutdown_tx: broadcast::Sender<()>,
}

async fn execute_post_queue_action(
    state: Arc<AppState>,
    action: PostQueueAction,
) {
    match action {
        PostQueueAction::ExitApp => {
            // Signal graceful shutdown
            let _ = state.shutdown_tx.send(());
            // Don't call std::process::exit - let main() handle cleanup
        }
        PostQueueAction::Shutdown => {
            #[cfg(target_os = "windows")]
            {
                // Signal shutdown first
                let _ = state.shutdown_tx.send(());
                // Then execute system shutdown
                tokio::time::sleep(Duration::from_secs(2)).await;
                let _ = std::process::Command::new("shutdown")
                    .args(&["/s", "/t", "0"])
                    .spawn();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = state.shutdown_tx.send(());
                tokio::time::sleep(Duration::from_secs(2)).await;
                let _ = std::process::Command::new("shutdown")
                    .arg("-h")
                    .arg("now")
                    .spawn();
            }
        }
        // ... similar for Sleep, Hibernate
    }
}

// In main():
async fn main() -> Result<()> {
    let (shutdown_tx, _) = broadcast::channel(1);
    let state = Arc::new(AppState {
        // ... existing fields ...
        shutdown_tx: shutdown_tx.clone(),
    });
    
    // Spawn server
    let server_handle = tokio::spawn(async move {
        run_server(state.clone()).await
    });
    
    // Wait for shutdown signal
    let mut shutdown_rx = shutdown_tx.subscribe();
    let _ = shutdown_rx.recv().await;
    
    // Graceful shutdown
    tracing::info!("Shutting down gracefully...");
    
    // Wait for all downloads to complete or timeout
    tokio::time::timeout(
        Duration::from_secs(10),
        state.manager.shutdown().await
    ).await.ok();
    
    // Close database
    drop(state.database.lock().await);
    
    // Stop server
    server_handle.abort();
    
    tracing::info!("Shutdown complete");
    Ok(())
}
```

**Files to change:** `vajra-daemon/src/main.rs`

### 1.7 Fix String-Based Error Classification

**Problem:** Error types classified by substring matching is fragile.

**Current Code (`download_task.rs:390,398`):**
```rust
let msg = e.to_string();
if msg.contains("Cancelled") {
    p.status = TaskState::Cancelled;
} else if msg.contains("Paused") {
    p.status = TaskState::Paused;
} else {
    p.status = TaskState::Failed;
    p.error = Some(msg);
}
```

**Fix:** Use a proper error enum with `thiserror`.

```rust
// download_task.rs - Define error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Download cancelled")]
    Cancelled,
    
    #[error("Download paused")]
    Paused,
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("HTTP error: {status}")]
    Http { status: u16, message: String },
    
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    
    #[error("Virus detected")]
    VirusDetected,
    
    #[error("Disk full")]
    DiskFull,
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

// In run_download():
let result = download_inner(&tx, req, progress, ctrl_rx).await;

match result {
    Ok(bytes) => {
        // Success path
    }
    Err(e) => {
        // Use downcast to check error type
        if let Some(DownloadError::Cancelled) = e.downcast_ref::<DownloadError>() {
            p.status = TaskState::Cancelled;
        } else if let Some(DownloadError::Paused) = e.downcast_ref::<DownloadError>() {
            p.status = TaskState::Paused;
        } else {
            p.status = TaskState::Failed;
            p.error = Some(e.to_string());
        }
    }
}

// In download_inner(), return proper error types:
if let Some(signal) = ctrl_rx.try_recv().ok() {
    match signal {
        ControlSignal::Cancel => return Err(DownloadError::Cancelled.into()),
        ControlSignal::Pause => return Err(DownloadError::Paused.into()),
    }
}
```

**Files to change:** 
- `vajra-engine/src/download_task.rs` (error enum + usage)
- `vajra-engine/src/multiplexer.rs` (return proper errors)
- `vajra-engine/src/hls.rs` (return proper errors)
- `vajra-engine/src/ftp_task.rs` (return proper errors)
- `vajra-engine/src/torrent_task.rs` (return proper errors)

---

## 2. Security Hardening Implementation (Completed)

### 2.1 Encrypt Vault Credentials

**Current:** Credentials stored in plaintext SQLite.

**Implementation Plan:**

```rust
// Add to vajra-engine/Cargo.toml:
[dependencies]
keyring = "3"
aes-gcm = "0.10"
argon2 = "0.5"

// Create new file: vajra-engine/src/vault.rs

use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;

pub struct EncryptedVault {
    cipher: Aes256Gcm,
    db: Database,
}

impl EncryptedVault {
    pub fn new(db: Database) -> Result<Self, VaultError> {
        // Try to load encryption key from system keyring
        let key = match Self::load_key_from_keyring()? {
            Some(key) => key,
            None => {
                // First run - generate and store key
                let key = Self::generate_and_store_key()?;
                key
            }
        };
        
        let cipher = Aes256Gcm::new(&key);
        Ok(Self { cipher, db })
    }
    
    fn load_key_from_keyring() -> Result<Option<[u8; 32]>, VaultError> {
        let entry = keyring::Entry::new("vajra", "vault_encryption_key")?;
        match entry.get_password() {
            Ok(key_base64) => {
                let key_bytes = base64::decode(&key_base64)?;
                if key_bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&key_bytes);
                    Ok(Some(key))
                } else {
                    Ok(None)
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    fn generate_and_store_key() -> Result<[u8; 32], VaultError> {
        let mut key = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut key);
        
        let key_base64 = base64::encode(&key);
        let entry = keyring::Entry::new("vajra", "vault_encryption_key")?;
        entry.set_password(&key_base64)?;
        
        Ok(key)
    }
    
    pub fn add_credential(&self, cred: VaultCredential) -> Result<(), VaultError> {
        // Encrypt username and password
        let encrypted_username = self.encrypt(&cred.username)?;
        let encrypted_password = self.encrypt(&cred.password)?;
        
        // Store encrypted values
        self.db.add_credential_encrypted(
            &cred.domain,
            &encrypted_username,
            &encrypted_password,
        )?;
        
        Ok(())
    }
    
    pub fn get_credential(&self, domain: &str) -> Result<Option<VaultCredential>, VaultError> {
        if let Some((enc_user, enc_pass)) = self.db.get_credential_encrypted(domain)? {
            let username = self.decrypt(&enc_user)?;
            let password = self.decrypt(&enc_pass)?;
            
            Ok(Some(VaultCredential {
                domain: domain.to_string(),
                username,
                password,
            }))
        } else {
            Ok(None)
        }
    }
    
    fn encrypt(&self, plaintext: &str) -> Result<String, VaultError> {
        let nonce = Nonce::from_slice(&Self::generate_nonce());
        let ciphertext = self.cipher.encrypt(nonce, plaintext.as_bytes())?;
        Ok(base64::encode(&ciphertext))
    }
    
    fn decrypt(&self, ciphertext_b64: &str) -> Result<String, VaultError> {
        let ciphertext = base64::decode(ciphertext_b64)?;
        let nonce = Nonce::from_slice(&ciphertext[..12]);
        let plaintext = self.cipher.decrypt(nonce, &ciphertext[12..])?;
        Ok(String::from_utf8(plaintext)?)
    }
    
    fn generate_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }
}
```

**Database Schema Change:**
```sql
-- Existing table remains for backward compatibility
-- New encrypted columns
ALTER TABLE vault_credentials ADD COLUMN username_encrypted TEXT;
ALTER TABLE vault_credentials ADD COLUMN password_encrypted TEXT;

-- Migration script
UPDATE vault_credentials 
SET username_encrypted = username,  -- Will be re-encrypted on first access
    password_encrypted = password
WHERE username_encrypted IS NULL;
```

**Migration Strategy:**
1. Add encrypted columns
2. On first access, re-encrypt all existing credentials
3. Remove plaintext columns after successful migration
4. Add rollback mechanism in case of keyring access failure

### 2.2 Add Daemon Authentication

**Implementation:**

```rust
// vajra-protocol/src/lib.rs - Add auth token to config

pub struct DaemonConfig {
    // ... existing fields ...
    pub auth_token: Option<String>,  // Generated on first launch
}

// vajra-daemon/src/main.rs - Add auth middleware

use axum::{
    middleware,
    extract::Request,
    http::{HeaderMap, StatusCode},
};

async fn auth_middleware(
    state: Arc<AppState>,
    req: Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Skip auth for health endpoint
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }
    
    // Check for Bearer token
    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());
    
    if let Some(auth) = auth_header {
        if auth.starts_with("Bearer ") {
            let token = &auth[7..];
            if let Some(expected_token) = &state.config.auth_token {
                if token == expected_token {
                    return Ok(next.run(req).await);
                }
            }
        }
    }
    
    Err(StatusCode::UNAUTHORIZED)
}

// In router setup:
let app = Router::new()
    .route("/health", get(health))
    .nest("/api/v1", api_routes)
    .layer(middleware::from_fn(auth_middleware));

// Generate token on first launch:
fn generate_auth_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let token: String = (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    token
}
```

**Client-Side Integration:**

```typescript
// vajra-ui-tauri/src/api.ts - Add token to all requests

async function apiRequest(path: string, options: RequestInit = {}) {
    const config = await loadConfig();
    const headers = new Headers(options.headers);
    
    if (config.auth_token) {
        headers.set('Authorization', `Bearer ${config.auth_token}`);
    }
    
    const response = await fetch(`http://127.0.0.1:6277${path}`, {
        ...options,
        headers,
    });
    
    if (response.status === 401) {
        throw new Error('Authentication failed');
    }
    
    return response;
}
```

**Browser Extension:**

```javascript
// browser-extension/background.js - Store and use token

async function initAuth() {
    // Read token from config file
    const configPath = await getConfigPath();
    const config = JSON.parse(await readFile(configPath));
    
    if (config.auth_token) {
        await chrome.storage.local.set({ authToken: config.auth_token });
    }
}

// Add to all API calls
async function callDaemon(endpoint, options = {}) {
    const { authToken } = await chrome.storage.local.get('authToken');
    
    const headers = {
        ...options.headers,
        'Authorization': `Bearer ${authToken}`,
    };
    
    return fetch(`http://127.0.0.1:6277${endpoint}`, {
        ...options,
        headers,
    });
}
```

### 2.3 Restrict CORS Origins

**Current:**
```rust
let cors = CorsLayer::permissive();
```

**Fix:**
```rust
use tower_http::cors::{CorsLayer, Any};
use http::Method;

let cors = CorsLayer::new()
    .allow_origin(["http://127.0.0.1:6277", "http://localhost:6277"])
    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
    .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]);
```

---

## 3. Feature Implementation Proposals (Completed)

### 3.1 Multi-Source Mirror Downloading

**Architecture:**

```rust
// vajra-engine/src/mirror.rs

use std::collections::HashMap;

pub struct MirrorSet {
    pub urls: Vec<Mirror>,
    pub health: HashMap<String, MirrorHealth>,
}

pub struct Mirror {
    pub url: String,
    pub priority: u8,
    pub enabled: bool,
}

pub struct MirrorHealth {
    pub latency_ms: u64,
    pub supports_ranges: bool,
    pub speed_bps: f64,
    pub last_checked: DateTime<Utc>,
    pub consecutive_failures: u32,
}

pub struct MirrorManager {
    mirrors: MirrorSet,
    client: reqwest::Client,
}

impl MirrorManager {
    pub async fn probe_all_mirrors(&self) -> Vec<(String, MirrorHealth)> {
        let mut results = Vec::new();
        
        for mirror in &self.mirrors.urls {
            if !mirror.enabled {
                continue;
            }
            
            let health = self.probe_mirror(&mirror.url).await;
            results.push((mirror.url.clone(), health));
        }
        
        results
    }
    
    async fn probe_mirror(&self, url: &str) -> MirrorHealth {
        let start = Instant::now();
        
        // Send HEAD request
        let result = self.client.head(url).send().await;
        
        let latency = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(resp) => {
                let supports_ranges = resp.headers()
                    .get("Accept-Ranges")
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.contains("bytes"))
                    .unwrap_or(false);
                
                MirrorHealth {
                    latency_ms: latency,
                    supports_ranges,
                    speed_bps: 0.0, // Will be measured during download
                    last_checked: Utc::now(),
                    consecutive_failures: 0,
                }
            }
            Err(_) => {
                MirrorHealth {
                    latency_ms: u64::MAX,
                    supports_ranges: false,
                    speed_bps: 0.0,
                    last_checked: Utc::now(),
                    consecutive_failures: 1,
                }
            }
        }
    }
    
    pub fn rank_mirrors(&self, results: &[(String, MirrorHealth)]) -> Vec<String> {
        let mut ranked = results.to_vec();
        
        // Sort by: supports_ranges (bool), latency_ms, speed_bps
        ranked.sort_by(|a, b| {
            let a_score = (a.1.supports_ranges as u8, a.1.speed_bps, 1.0 / a.1.latency_ms as f64);
            let b_score = (b.1.supports_ranges as u8, b.1.speed_bps, 1.0 / b.1.latency_ms as f64);
            b_score.partial_cmp(&a_score).unwrap()
        });
        
        ranked.into_iter().map(|(url, _)| url).collect()
    }
    
    pub async fn assign_chunks_to_mirrors(
        &self,
        chunks: &[Chunk],
        ranked_mirrors: &[String],
    ) -> HashMap<usize, String> {
        let mut assignments = HashMap::new();
        
        for (i, chunk) in chunks.iter().enumerate() {
            // Round-robin assignment to top 3 mirrors
            let mirror_idx = i % ranked_mirrors.len().min(3);
            assignments.insert(chunk.id, ranked_mirrors[mirror_idx].clone());
        }
        
        assignments
    }
    
    pub async fn handle_mirror_failure(
        &mut self,
        mirror_url: &str,
        chunk_id: usize,
    ) -> Option<String> {
        // Mark mirror as failed
        if let Some(health) = self.mirrors.health.get_mut(mirror_url) {
            health.consecutive_failures += 1;
            
            if health.consecutive_failures >= 3 {
                health.enabled = false; // Disable mirror after 3 failures
            }
        }
        
        // Re-rank remaining mirrors
        let ranked = self.rank_mirrors(&self.probe_all_mirrors().await);
        
        // Return new mirror for the chunk
        ranked.first().cloned()
    }
}
```

**Metalink Support:**

```rust
// vajra-engine/src/metalink.rs

use quick_xml::Reader;
use quick_xml::events::Event;

pub struct Metalink {
    pub files: Vec<MetalinkFile>,
}

pub struct MetalinkFile {
    pub name: String,
    pub size: u64,
    pub hashes: HashMap<String, String>,
    pub urls: Vec<MetalinkUrl>,
}

pub struct MetalinkUrl {
    pub url: String,
    pub priority: u8,
    pub location: Option<String>,
}

pub fn parse_metalink(xml: &str) -> Result<Metalink, MetalinkError> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    
    let mut metalink = Metalink { files: Vec::new() };
    let mut current_file = None;
    let mut current_url = None;
    let mut in_hashes = false;
    
    let mut buf = Vec::new();
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"file" => {
                    if let Some(name_attr) = e.attributes()
                        .find(|a| a.as_ref().ok().and_then(|a| 
                            std::str::from_utf8(a.key.as_ref()).ok() == Some("name")
                        ))
                    {
                        current_file = Some(MetalinkFile {
                            name: String::new(),
                            size: 0,
                            hashes: HashMap::new(),
                            urls: Vec::new(),
                        });
                    }
                }
                b"url" => {
                    current_url = Some(MetalinkUrl {
                        url: String::new(),
                        priority: 99,
                        location: None,
                    });
                }
                b"hash" => {
                    in_hashes = true;
                }
                _ => {}
            },
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().ok().map(|s| s.to_string());
                
                if let Some(text) = text {
                    if in_hashes {
                        // Add to current file's hashes
                    } else if let Some(ref mut url) = current_url {
                        url.url = text;
                    } else if let Some(ref mut file) = current_file {
                        // Check if this is size, name, etc.
                        // Parse and set appropriate field
                    }
                }
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"file" => {
                    if let Some(file) = current_file.take() {
                        metalink.files.push(file);
                    }
                }
                b"url" => {
                    if let Some(url) = current_url.take() {
                        if let Some(ref mut file) = current_file {
                            file.urls.push(url);
                        }
                    }
                }
                b"hash" => {
                    in_hashes = false;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(MetalinkError::Parse(e)),
            _ => {}
        }
        buf.clear();
    }
    
    Ok(metalink)
}
```

### 3.2 RSS/Podcast Feed Support

**Implementation:**

```rust
// vajra-engine/src/feed.rs

use feed_rs::parser;
use chrono::{DateTime, Utc};

pub struct FeedManager {
    feeds: Vec<FeedSubscription>,
    db: Database,
}

pub struct FeedSubscription {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub last_checked: DateTime<Utc>,
    pub auto_download: bool,
    pub filters: FeedFilters,
}

pub struct FeedFilters {
    pub title_regex: Option<String>,
    pub min_size_mb: Option<u64>,
    pub max_age_days: Option<u32>,
    pub file_extensions: Vec<String>,
}

pub struct FeedItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub link: String,
    pub enclosure: Option<FeedEnclosure>,
    pub published: DateTime<Utc>,
}

pub struct FeedEnclosure {
    pub url: String,
    pub mime_type: String,
    pub size: u64,
}

impl FeedManager {
    pub async fn check_feed(&self, sub: &FeedSubscription) -> Result<Vec<FeedItem>, FeedError> {
        let response = reqwest::get(&sub.url).await?;
        let xml = response.text().await?;
        
        let feed = parser::parse(xml.as_bytes())
            .map_err(|e| FeedError::Parse(e.to_string()))?;
        
        let mut items = Vec::new();
        
        for entry in feed.entries {
            // Apply filters
            if !self.matches_filters(&entry, &sub.filters) {
                continue;
            }
            
            // Check if already downloaded
            if self.db.is_feed_item_downloaded(sub.id, &entry.id).await? {
                continue;
            }
            
            items.push(FeedItem {
                id: entry.id.clone(),
                title: entry.title.map(|t| t.content).unwrap_or_default(),
                description: entry.summary.map(|s| s.content).unwrap_or_default(),
                link: entry.links.first()
                    .map(|l| l.href.clone())
                    .unwrap_or_default(),
                enclosure: entry.media.first()
                    .and_then(|m| m.content.as_ref())
                    .map(|c| FeedEnclosure {
                        url: c.url.clone(),
                        mime_type: c.content_type.clone(),
                        size: c.size.unwrap_or(0),
                    }),
                published: entry.published.unwrap_or(entry.updated),
            });
        }
        
        Ok(items)
    }
    
    fn matches_filters(&self, entry: &feed_rs::model::Entry, filters: &FeedFilters) -> bool {
        // Title regex filter
        if let Some(regex) = &filters.title_regex {
            let re = regex::Regex::new(regex).ok();
            if let Some(re) = re {
                let title = entry.title.as_ref().map(|t| t.content.as_str()).unwrap_or("");
                if !re.is_match(title) {
                    return false;
                }
            }
        }
        
        // Size filter
        if let Some(min_size) = filters.min_size_mb {
            let size = entry.media.first()
                .and_then(|m| m.content.as_ref())
                .and_then(|c| c.size)
                .unwrap_or(0);
            
            if size < min_size * 1024 * 1024 {
                return false;
            }
        }
        
        // Age filter
        if let Some(max_age) = filters.max_age_days {
            let published = entry.published.unwrap_or(entry.updated);
            let age = Utc::now() - published;
            if age.num_days() > max_age as i64 {
                return false;
            }
        }
        
        // Extension filter
        if !filters.file_extensions.is_empty() {
            let url = entry.links.first().map(|l| l.href.as_str()).unwrap_or("");
            let ext = url.rsplit('.').next().unwrap_or("");
            if !filters.file_extensions.contains(&ext.to_lowercase()) {
                return false;
            }
        }
        
        true
    }
    
    pub async fn auto_download_matching(&self, items: &[FeedItem], sub: &FeedSubscription) {
        for item in items {
            if let Some(enclosure) = &item.enclosure {
                let req = DownloadRequest {
                    url: enclosure.url.clone(),
                    filename: Some(format!("{}.{}", item.title, self.guess_extension(&enclosure.mime_type))),
                    dest_path: Some(self.get_feed_output_dir(sub).await),
                    ..Default::default()
                };
                
                if let Err(e) = self.manager.add(req).await {
                    tracing::warn!("Failed to auto-download feed item: {}", e);
                }
            }
        }
    }
}
```

### 3.3 Adaptive Chunk Sizing

**Implementation:**

```rust
// vajra-engine/src/multiplexer.rs - Add adaptive sizing

pub struct AdaptiveSizer {
    initial_chunk_size: u64,
    min_chunk_size: u64,
    max_chunk_size: u64,
    growth_factor: f64,
    shrink_threshold: f64,
}

impl AdaptiveSizer {
    pub fn new(total_size: u64, initial_connections: usize) -> Self {
        // Start with 1MB chunks, grow up to 16MB
        let initial_chunk_size = 1024 * 1024; // 1MB
        let max_chunk_size = 16 * 1024 * 1024; // 16MB
        let min_chunk_size = 128 * 1024; // 128KB
        
        Self {
            initial_chunk_size,
            min_chunk_size,
            max_chunk_size,
            growth_factor: 2.0,
            shrink_threshold: 0.5, // Shrink if speed < 50% of max
        }
    }
    
    pub fn calculate_initial_chunks(
        &self,
        total_size: u64,
        max_connections: usize,
    ) -> Vec<Chunk> {
        // Start with fewer, larger chunks if file is big enough
        let target_chunk_size = self.initial_chunk_size;
        let num_chunks = (total_size / target_chunk_size)
            .min(max_connections as u64)
            .max(1) as usize;
        
        calculate_chunks(total_size, num_chunks)
    }
    
    pub fn adjust_chunk_size(
        &mut self,
        current_speed_bps: f64,
        max_observed_speed: f64,
        chunk_count: usize,
    ) -> ChunkSizeAction {
        // If we're at 80%+ of max speed, try larger chunks
        if current_speed_bps > max_observed_speed * 0.8 {
            let new_size = (self.initial_chunk_size as f64 * self.growth_factor) as u64;
            if new_size <= self.max_chunk_size {
                self.initial_chunk_size = new_size;
                return ChunkSizeAction::Increase {
                    new_chunk_size: new_size,
                    suggested_connections: (chunk_count as f64 * 0.8) as usize,
                };
            }
        }
        
        // If we're below threshold, try smaller chunks
        if current_speed_bps < max_observed_speed * self.shrink_threshold {
            let new_size = (self.initial_chunk_size as f64 / self.growth_factor) as u64;
            if new_size >= self.min_chunk_size {
                self.initial_chunk_size = new_size;
                return ChunkSizeAction::Decrease {
                    new_chunk_size: new_size,
                    suggested_connections: (chunk_count as f64 * 1.2) as usize,
                };
            }
        }
        
        ChunkSizeAction::NoChange
    }
}

pub enum ChunkSizeAction {
    Increase {
        new_chunk_size: u64,
        suggested_connections: usize,
    },
    Decrease {
        new_chunk_size: u64,
        suggested_connections: usize,
    },
    NoChange,
}
```

### 3.4 Import/Export Functionality

**Implementation:**

```rust
// vajra-protocol/src/lib.rs - Add import/export types

pub struct ExportData {
    pub version: String,
    pub exports: Vec<ExportItem>,
    pub config: Option<DaemonConfig>,
    pub vault: Option<Vec<VaultCredential>>,
}

pub struct ExportItem {
    pub url: String,
    pub filename: String,
    pub dest_path: PathBuf,
    pub status: String,
    pub total_bytes: u64,
    pub bytes_done: u64,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

// vajra-daemon/src/api/handlers.rs

pub async fn export_data(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ExportParams>,
) -> Result<Json<ExportData>, DaemonError> {
    let db = state.database.lock().await;
    
    let mut exports = Vec::new();
    
    // Export jobs
    if params.include_active {
        let jobs = db.load_all_jobs().await?;
        for job in jobs {
            exports.push(ExportItem {
                url: job.request.url.clone(),
                filename: job.request.filename.clone().unwrap_or_default(),
                dest_path: job.request.dest_path.clone().unwrap_or_default(),
                status: job.state.clone(),
                total_bytes: 0, // Will be filled from progress
                bytes_done: 0,
                created_at: job.created_at,
                metadata: HashMap::new(),
            });
        }
    }
    
    // Export history
    if params.include_history {
        let history = db.get_history(0, 10000).await?;
        for entry in history {
            exports.push(ExportItem {
                url: entry.url.clone(),
                filename: entry.filename.clone(),
                dest_path: entry.dest_path.clone(),
                status: entry.status.clone(),
                total_bytes: entry.total_bytes,
                bytes_done: entry.total_bytes,
                created_at: entry.completed_at,
                metadata: HashMap::new(),
            });
        }
    }
    
    // Export config if requested
    let config = if params.include_config {
        Some(state.config.clone())
    } else {
        None
    };
    
    // Export vault if requested (encrypted)
    let vault = if params.include_vault {
        Some(db.get_credentials().await?)
    } else {
        None
    };
    
    Ok(Json(ExportData {
        version: env!("CARGO_PKG_VERSION").to_string(),
        exports,
        config,
        vault,
    }))
}

pub async fn import_data(
    State(state): State<Arc<AppState>>,
    Json(data): Json<ExportData>,
) -> Result<Json<ImportResult>, DaemonError> {
    let mut result = ImportResult {
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    
    let db = state.database.lock().await;
    
    // Import jobs
    for item in &data.exports {
        if item.status == "complete" || item.status == "completed" {
            // Import as history
            if let Err(e) = db.insert_history(&HistoryEntry {
                id: Uuid::new_v4().to_string(),
                url: item.url.clone(),
                filename: item.filename.clone(),
                dest_path: item.dest_path.clone(),
                total_bytes: item.total_bytes,
                speed_avg: 0,
                status: item.status.clone(),
                completed_at: item.created_at,
            }).await {
                result.errors.push(format!("Failed to import {}: {}", item.url, e));
            } else {
                result.imported += 1;
            }
        } else {
            // Import as active job
            let req = DownloadRequest {
                url: item.url.clone(),
                filename: Some(item.filename.clone()),
                dest_path: Some(item.dest_path.clone()),
                ..Default::default()
            };
            
            match state.manager.add(req).await {
                Ok(_) => result.imported += 1,
                Err(e) => {
                    result.errors.push(format!("Failed to import {}: {}", item.url, e));
                    result.skipped += 1;
                }
            }
        }
    }
    
    // Import config if present
    if let Some(config) = data.config {
        // Merge or replace config
        // ...
    }
    
    // Import vault if present
    if let Some(vault) = data.vault {
        for cred in vault {
            if let Err(e) = db.add_credential(&cred).await {
                result.errors.push(format!("Failed to import vault entry: {}", e));
            }
        }
    }
    
    Ok(Json(result))
}
```

### 3.5 Auto-Update System

**Implementation:**

```rust
// Add to vajra-ui-tauri/src-tauri/Cargo.toml:
[dependencies]
tauri-plugin-updater = "2"

// tauri.conf.json - Add updater config:
{
  "plugins": {
    "updater": {
      "active": true,
      "pubkey": "YOUR_PUBLIC_KEY_HERE",
      "endpoints": [
        "https://github.com/yourusername/vajra/releases/latest/download/latest.json"
      ]
    }
  }
}

// vajra-ui-tauri/src-tauri/src/lib.rs:
use tauri_plugin_updater::UpdaterExt;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let updater = app.updater()?;
            
            // Check for updates on startup
            tauri::async_runtime::spawn(async move {
                if let Ok(Some(update)) = updater.check().await {
                    // Notify user
                    let _ = app.emit("update-available", update.version.clone());
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // ... existing commands ...
            check_for_updates,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    
    match updater.check().await {
        Ok(Some(update)) => {
            Ok(Some(UpdateInfo {
                version: update.version.clone(),
                date: update.date.clone(),
                body: update.body.clone(),
            }))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    
    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
        let on_progress = move |chunk_length: usize, content_length: Option<u64>| {
            let _ = app.emit("update-progress", UpdateProgress {
                downloaded: chunk_length,
                total: content_length,
            });
        };
        
        update.download_and_install(on_progress).await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}
```

---

## 4. Architecture Evolution Proposals (Completed)

### 4.1 Plugin System Architecture

**Design:**

```rust
// vajra-engine/src/plugin.rs

use std::sync::Arc;
use tokio::sync::RwLock;

pub trait DownloadPlugin: Send + Sync {
    /// Plugin metadata
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    
    /// Lifecycle hooks
    fn on_download_added(&self, req: &DownloadRequest) -> Result<DownloadRequest, PluginError>;
    fn on_download_completed(&self, path: &Path) -> Result<(), PluginError>;
    fn on_download_failed(&self, error: &str) -> Result<(), PluginError>;
    
    /// Custom protocol handler (optional)
    fn supported_schemes(&self) -> Vec<String>;
    fn handle_download(&self, req: &DownloadRequest) -> Option<DownloadResult>;
    
    /// Post-processing extension (optional)
    fn post_process(&self, path: &Path) -> Result<PostProcessResult, PluginError>;
}

pub struct PluginManager {
    plugins: Vec<Arc<dyn DownloadPlugin>>,
}

impl PluginManager {
    pub async fn register_plugin<P: DownloadPlugin + 'static>(&mut self, plugin: P) {
        self.plugins.push(Arc::new(plugin));
    }
    
    pub async fn load_plugin_from_file(&mut self, path: &Path) -> Result<(), PluginError> {
        // Load .so/.dll/.dylib dynamically
        #[cfg(target_os = "linux")]
        let lib = libloading::Library::new(path)?;
        
        // Look for plugin entry point
        let init_fn: libloading::Symbol<fn() -> Box<dyn DownloadPlugin>> = 
            unsafe { lib.get(b"create_plugin")? };
        
        let plugin = init_fn();
        self.plugins.push(Arc::from(plugin));
        
        Ok(())
    }
    
    pub async fn notify_added(&self, req: &DownloadRequest) -> DownloadRequest {
        let mut current = req.clone();
        
        for plugin in &self.plugins {
            match plugin.on_download_added(&current) {
                Ok(modified) => current = modified,
                Err(e) => tracing::warn!("Plugin {} rejected download: {}", plugin.name(), e),
            }
        }
        
        current
    }
    
    pub async fn notify_completed(&self, path: &Path) {
        for plugin in &self.plugins {
            if let Err(e) = plugin.on_download_completed(path) {
                tracing::warn!("Plugin {} error on completion: {}", plugin.name(), e);
            }
        }
    }
    
    pub fn find_protocol_handler(&self, scheme: &str) -> Option<Arc<dyn DownloadPlugin>> {
        self.plugins.iter()
            .find(|p| p.supported_schemes().contains(&scheme.to_string()))
            .cloned()
    }
}

// Example plugin: Auto-categorizer

pub struct AutoCategorizerPlugin {
    rules: Vec<CategoryRule>,
}

impl DownloadPlugin for AutoCategorizerPlugin {
    fn on_download_added(&self, req: &DownloadRequest) -> Result<DownloadRequest, PluginError> {
        let mut req = req.clone();
        
        for rule in &self.rules {
            if rule.matches(&req) {
                req.dest_path = Some(rule.output_dir.clone());
                req.metadata.insert("category".to_string(), rule.category.clone());
                break;
            }
        }
        
        Ok(req)
    }
    
    // ... other methods
}
```

### 4.2 Event-Driven Architecture

**Current:** Direct function calls, tight coupling.

**Proposed:** Event bus for loose coupling.

```rust
// vajra-engine/src/events.rs

use tokio::sync::broadcast;

pub enum DownloadEvent {
    Added { id: Uuid, request: DownloadRequest },
    Started { id: Uuid },
    Progress { id: Uuid, bytes_done: u64, speed_bps: f64 },
    Paused { id: Uuid },
    Resumed { id: Uuid },
    Completed { id: Uuid, path: PathBuf, bytes: u64 },
    Failed { id: Uuid, error: String },
    Cancelled { id: Uuid },
}

pub struct EventBus {
    tx: broadcast::Sender<DownloadEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self { tx }
    }
    
    pub fn publish(&self, event: DownloadEvent) {
        let _ = self.tx.send(event);
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.tx.subscribe()
    }
}

// Usage in DownloadManager:
impl DownloadManager {
    pub async fn add(&self, req: DownloadRequest) -> Result<Uuid> {
        let id = Uuid::new_v4();
        
        // ... add logic ...
        
        self.events.publish(DownloadEvent::Added {
            id,
            request: req.clone(),
        });
        
        Ok(id)
    }
    
    pub async fn start_scheduler(self: Arc<Self>) {
        let mut event_rx = self.events.subscribe();
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = self.tick() => {}
                    Ok(event) = event_rx.recv() => {
                        match event {
                            DownloadEvent::Completed { id, .. } => {
                                self.on_download_completed(id).await;
                            }
                            DownloadEvent::Failed { id, error } => {
                                self.on_download_failed(id, &error).await;
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    }
}
```

---

## 5. Performance Optimization Deep-Dive (Completed)

### 5.1 HTTP/3 (QUIC) Support

**Implementation Plan:**

```rust
// Add to Cargo.toml:
[dependencies]
h3 = "0.0.6"
h3-quinn = "0.0.7"
quinn = "0.11"

// vajra-engine/src/download_task.rs - Add HTTP/3 client

use quinn::{ClientConfig, Endpoint};
use h3::client::SendRequest;

pub struct Http3Client {
    endpoint: Endpoint,
}

impl Http3Client {
    pub async fn new() -> Result<Self> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(ClientConfig::with_root_certificates());
        
        Ok(Self { endpoint })
    }
    
    pub async fn download_with_http3(&self, url: &str) -> Result<Vec<u8>> {
        let url = url::Url::parse(url)?;
        let host = url.host_str().ok_or_else(|| anyhow!("No host in URL"))?;
        
        // Connect to server
        let connection = self.endpoint
            .connect(format!("{}:443", host).parse()?, host)?
            .await?;
        
        // Create HTTP/3 connection
        let mut send_request = h3::client::send_request::handshake(connection).await?;
        
        // Send request
        let request = http::Request::builder()
            .uri(url.as_str())
            .body(())?;
        
        let stream = send_request.send_request(request).await?;
        
        // Read response
        let mut response_body = Vec::new();
        // ... read body stream ...
        
        Ok(response_body)
    }
}

// In run_download(), try HTTP/3 first if URL is HTTPS:
if url.starts_with("https://") {
    match self.http3_client.download_with_http3(&url).await {
        Ok(data) => return Ok(data),
        Err(e) => {
            tracing::info!("HTTP/3 failed, falling back to HTTP/2: {}", e);
            // Fall back to HTTP/2
        }
    }
}
```

### 5.2 io_uring on Linux

**Implementation:**

```rust
// Add to Cargo.toml (Linux only):
[target.'cfg(target_os = "linux")'.dependencies]
io-uring = "0.6"

// vajra-engine/src/writer.rs - io_uring writer for Linux

#[cfg(target_os = "linux")]
pub async fn start_disk_writer_uring(
    file: Arc<std::fs::File>,
    mut rx: mpsc::Receiver<DataFrame>,
) -> Result<()> {
    use io_uring::{IoUring, types, cqueue, squeue};
    
    let mut ring = IoUing::builder().build(256)?;
    let fd = types::Fd(file.as_raw_fd());
    
    let mut pending_writes = std::collections::HashMap::new();
    
    while let Some(frame) = rx.recv().await {
        // Submit write operation
        let write_op = types::Write::new(fd, frame.payload.as_ptr(), frame.payload.len() as u32)
            .offset(frame.absolute_offset);
        
        let entry = squeue::Entry::new(write_op)
            .user_data(frame.absolute_offset); // Use offset as user data
        
        unsafe {
            ring.submission().push(&entry)?;
        }
        ring.submit()?;
        
        pending_writes.insert(frame.absolute_offset, frame.payload.len());
        
        // Reap completions
        ring.submit_and_wait(1)?;
        
        for cqe in ring.completion() {
            let offset = cqe.user_data();
            let result = cqe.result();
            
            if result < 0 {
                return Err(std::io::Error::from_raw_os_error(-result).into());
            }
            
            pending_writes.remove(&offset);
        }
    }
    
    // Sync to disk
    let sync_op = types::Fsync::new(fd);
    let entry = squeue::Entry::new(sync_op);
    unsafe { ring.submission().push(&entry)?; }
    ring.submit()?;
    
    Ok(())
}
```

### 5.3 Memory-Mapped Files for Small Downloads

**Implementation:**

```rust
// Add to Cargo.toml:
[dependencies]
memmap2 = "0.9"

// vajra-engine/src/writer.rs

use memmap2::MmapMut;

pub async fn write_small_file_mmap(
    path: &Path,
    size: u64,
    mut rx: mpsc::Receiver<DataFrame>,
) -> Result<()> {
    // For files < 64MB, use mmap
    if size > 64 * 1024 * 1024 {
        return Err(anyhow!("File too large for mmap"));
    }
    
    // Create and size the file
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)?;
    file.set_len(size)?;
    
    // Memory-map the file
    let mut mmap = unsafe { MmapMut::map_mut(&file)? };
    
    // Write frames directly to mmap
    while let Some(frame) = rx.recv().await {
        let offset = frame.absolute_offset as usize;
        let end = offset + frame.payload.len();
        
        if end > mmap.len() {
            return Err(anyhow!("Write exceeds file size"));
        }
        
        mmap[offset..end].copy_from_slice(&frame.payload);
    }
    
    // Flush to disk
    mmap.flush()?;
    
    Ok(())
}

// In download_task.rs, choose writer based on file size:
if total_size < 64 * 1024 * 1024 {
    write_small_file_mmap(&dest_path, total_size, writer_rx).await?;
} else {
    start_disk_writer(&file, writer_rx).await?;
}
```

---

## 6. Testing Strategy with Examples (Completed)

### 6.1 Integration Test: Download Lifecycle

```rust
// vajra-engine/tests/download_lifecycle.rs

use vajra_engine::{DownloadManager, DownloadRequest, TaskState};
use tempfile::TempDir;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_add_pause_resume_complete() {
    // Setup mock server
    let mock_server = MockServer::start().await;
    
    // Create a 10MB test file
    let file_data = vec![42u8; 10 * 1024 * 1024];
    
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200)
            .insert_header("Content-Length", "10485760")
            .insert_header("Accept-Ranges", "bytes"))
        .mount(&mock_server)
        .await;
    
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(206)
            .insert_header("Content-Range", "bytes 0-10485759/10485760")
            .set_body_bytes(file_data.clone()))
        .mount(&mock_server)
        .await;
    
    // Create download manager
    let temp_dir = TempDir::new()?;
    let manager = DownloadManager::new(temp_dir.path().to_path_buf()).await?;
    
    // Add download
    let req = DownloadRequest {
        url: mock_server.uri(),
        filename: Some("test.bin".to_string()),
        dest_path: Some(temp_dir.path().to_path_buf()),
        max_connections: Some(4),
        ..Default::default()
    };
    
    let id = manager.add(req).await?;
    
    // Wait for download to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify it's downloading
    let progress = manager.progress(id).await?;
    assert!(matches!(progress.status, TaskState::Downloading));
    assert!(progress.bytes_done > 0);
    
    // Pause
    manager.pause(id).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let progress = manager.progress(id).await?;
    assert!(matches!(progress.status, TaskState::Paused));
    assert!(progress.bytes_done < 10 * 1024 * 1024);
    
    // Resume
    manager.resume(id).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let progress = manager.progress(id).await?;
    assert!(matches!(progress.status, TaskState::Downloading));
    
    // Wait for completion
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    let progress = manager.progress(id).await?;
    assert!(matches!(progress.status, TaskState::Completed));
    assert_eq!(progress.bytes_done, 10 * 1024 * 1024);
    
    // Verify file exists and has correct content
    let file_path = temp_dir.path().join("test.bin");
    assert!(file_path.exists());
    let file_data = std::fs::read(&file_path)?;
    assert_eq!(file_data.len(), 10 * 1024 * 1024);
    assert!(file_data.iter().all(|&b| b == 42));
}

#[tokio::test]
async fn test_resume_after_crash() {
    // Setup
    let mock_server = MockServer::start().await;
    let temp_dir = TempDir::new()?;
    
    // Create partial download state
    let partial_data = vec![42u8; 5 * 1024 * 1024]; // 5MB downloaded
    let state = DownloadState {
        id: Uuid::new_v4(),
        url: mock_server.uri(),
        total_bytes: 10 * 1024 * 1024,
        etag: Some("test-etag".to_string()),
        last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".to_string()),
        chunks: vec![ChunkProgress {
            chunk_id: 0,
            bytes_written: 5 * 1024 * 1024,
            start_byte: Some(0),
            end_byte: Some(10 * 1024 * 1024 - 1),
        }],
        paused_at: Utc::now(),
    };
    
    // Write state file
    let state_json = serde_json::to_string(&state)?;
    std::fs::write(
        temp_dir.path().join(".test.bin.vajra.state"),
        state_json,
    )?;
    
    // Write partial file
    std::fs::write(temp_dir.path().join("test.bin"), &partial_data)?;
    
    // Setup mock to expect resume request
    Mock::given(method("GET"))
        .and(path("/test.bin"))
        .respond_with(|req: &wiremock::Request| {
            let range_header = req.headers.get("Range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            
            if range_header.contains("bytes=5242880-") {
                ResponseTemplate::new(206)
                    .insert_header("Content-Range", "bytes 5242880-10485759/10485760")
                    .set_body_bytes(vec![42u8; 5 * 1024 * 1024])
                    .into_response()
            } else {
                ResponseTemplate::new(416).into_response()
            }
        })
        .mount(&mock_server)
        .await;
    
    // Create manager and restore job
    let manager = DownloadManager::new(temp_dir.path().to_path_buf()).await?;
    let req = DownloadRequest {
        url: mock_server.uri(),
        filename: Some("test.bin".to_string()),
        dest_path: Some(temp_dir.path().to_path_buf()),
        ..Default::default()
    };
    
    let id = manager.add(req).await?;
    
    // Wait for completion
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    let progress = manager.progress(id).await?;
    assert!(matches!(progress.status, TaskState::Completed));
    assert_eq!(progress.bytes_done, 10 * 1024 * 1024);
}
```

### 6.2 API Integration Tests

```rust
// vajra-daemon/tests/api_tests.rs

use axum::http::StatusCode;
use serde_json::json;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_add_download_api() {
    // Start test server
    let app = create_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);
    
    // Add download
    let response = client.post(&format!("{}/api/v1/downloads", base_url))
        .json(&json!({
            "url": "https://example.com/file.bin",
            "filename": "file.bin",
            "dest_path": "/tmp/downloads",
            "max_connections": 8
        }))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body: serde_json::Value = response.json().await?;
    let id = body["id"].as_str().unwrap();
    
    // Get download
    let response = client.get(&format!("{}/api/v1/downloads/{}", base_url, id))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let download: serde_json::Value = response.json().await?;
    assert_eq!(download["url"], "https://example.com/file.bin");
    assert_eq!(download["filename"], "file.bin");
    
    // Pause
    let response = client.patch(&format!("{}/api/v1/downloads/{}", base_url, id))
        .json(&json!({
            "action": "pause"
        }))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Delete
    let response = client.delete(&format!("{}/api/v1/downloads/{}", base_url, id))
        .query(&[("delete_file", "true")])
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_sse_events() {
    let app = create_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    let client = reqwest::Client::new();
    let base_url = format!("http://{}", addr);
    
    // Connect to SSE
    let mut response = client.get(&format!("{}/api/v1/events", base_url))
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("Content-Type").unwrap(),
        "text/event-stream"
    );
    
    // Add a download to trigger events
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        client.post(&format!("{}/api/v1/downloads", base_url))
            .json(&json!({
                "url": "https://example.com/file.bin"
            }))
            .send()
            .await
            .unwrap();
    });
    
    // Read SSE events
    let mut buffer = String::new();
    while let Some(chunk) = response.chunk().await? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        
        if buffer.contains("event: added") {
            break;
        }
    }
    
    assert!(buffer.contains("event: added"));
    assert!(buffer.contains("data:"));
}
```

### 6.3 Frontend Component Tests

```typescript
// vajra-ui-tauri/src/__tests__/DownloadsTable.test.tsx

import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { DownloadsTable } from '../components/DownloadsTable';

describe('DownloadsTable', () => {
    const mockDownloads = [
        {
            id: '1',
            filename: 'file1.bin',
            status: 'downloading',
            bytes_done: 5000000,
            total_bytes: 10000000,
            speed_bps: 1000000,
            eta_seconds: 5,
        },
        {
            id: '2',
            filename: 'file2.bin',
            status: 'completed',
            bytes_done: 20000000,
            total_bytes: 20000000,
            speed_bps: 0,
            eta_seconds: 0,
        },
    ];

    it('renders download list', () => {
        render(<DownloadsTable downloads={mockDownloads} />);
        
        expect(screen.getByText('file1.bin')).toBeInTheDocument();
        expect(screen.getByText('file2.bin')).toBeInTheDocument();
    });

    it('shows progress bar', () => {
        render(<DownloadsTable downloads={mockDownloads} />);
        
        const progressBar = screen.getByRole('progressbar');
        expect(progressBar).toBeInTheDocument();
    });

    it('handles row selection', () => {
        const onSelectionChange = vi.fn();
        render(
            <DownloadsTable 
                downloads={mockDownloads}
                onSelectionChange={onSelectionChange}
            />
        );
        
        const checkbox = screen.getByRole('checkbox', { name: /file1.bin/i });
        fireEvent.click(checkbox);
        
        expect(onSelectionChange).toHaveBeenCalledWith(['1']);
    });

    it('sorts by filename', () => {
        render(<DownloadsTable downloads={mockDownloads} />);
        
        const header = screen.getByText('File Name');
        fireEvent.click(header);
        
        // Verify sort order changed
        const rows = screen.getAllByRole('row');
        expect(rows[1]).toHaveTextContent('file1.bin');
    });
});
```

---

## 7. Frontend Refactoring Blueprint (Completed)

### 7.1 Zustand Store Structure

```typescript
// vajra-ui-tauri/src/stores/downloadStore.ts

import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface Download {
    id: string;
    url: string;
    filename: string;
    status: string;
    bytes_done: number;
    total_bytes: number;
    speed_bps: number;
    eta_seconds: number;
    segments: SegmentInfo[];
    dest_path: string;
    added_at: string;
    category?: string;
    tags?: string[];
}

interface DownloadStore {
    // State
    downloads: Record<string, Download>;
    selectedIds: Set<string>;
    sortBy: string;
    sortDirection: 'asc' | 'desc';
    filters: {
        status?: string[];
        category?: string;
        search?: string;
    };
    
    // Actions
    addDownload: (download: Download) => void;
    updateDownload: (id: string, updates: Partial<Download>) => void;
    removeDownload: (id: string) => void;
    
    selectDownload: (id: string, multiSelect?: boolean) => void;
    clearSelection: () => void;
    selectAll: () => void;
    
    setSortBy: (field: string) => void;
    setFilters: (filters: Partial<DownloadStore['filters']>) => void;
    
    // Computed
    getSortedDownloads: () => Download[];
    getFilteredDownloads: () => Download[];
}

export const useDownloadStore = create<DownloadStore>()(
    persist(
        (set, get) => ({
            // Initial state
            downloads: {},
            selectedIds: new Set(),
            sortBy: 'added_at',
            sortDirection: 'desc',
            filters: {},
            
            // Actions
            addDownload: (download) => set((state) => ({
                downloads: {
                    ...state.downloads,
                    [download.id]: download,
                },
            })),
            
            updateDownload: (id, updates) => set((state) => ({
                downloads: {
                    ...state.downloads,
                    [id]: {
                        ...state.downloads[id],
                        ...updates,
                    },
                },
            })),
            
            removeDownload: (id) => set((state) => {
                const { [id]: removed, ...rest } = state.downloads;
                const newSelected = new Set(state.selectedIds);
                newSelected.delete(id);
                return {
                    downloads: rest,
                    selectedIds: newSelected,
                };
            }),
            
            selectDownload: (id, multiSelect = false) => set((state) => {
                const newSelected = multiSelect 
                    ? new Set(state.selectedIds)
                    : new Set<string>();
                
                if (newSelected.has(id)) {
                    newSelected.delete(id);
                } else {
                    newSelected.add(id);
                }
                
                return { selectedIds: newSelected };
            }),
            
            clearSelection: () => set({ selectedIds: new Set() }),
            
            selectAll: () => set((state) => ({
                selectedIds: new Set(Object.keys(state.downloads)),
            })),
            
            setSortBy: (field) => set((state) => ({
                sortBy: field,
                sortDirection: state.sortBy === field && state.sortDirection === 'desc' 
                    ? 'asc' 
                    : 'desc',
            })),
            
            setFilters: (filters) => set((state) => ({
                filters: { ...state.filters, ...filters },
            })),
            
            // Computed
            getSortedDownloads: () => {
                const { downloads, sortBy, sortDirection } = get();
                const arr = Object.values(downloads);
                
                arr.sort((a, b) => {
                    const aVal = a[sortBy as keyof Download];
                    const bVal = b[sortBy as keyof Download];
                    
                    if (typeof aVal === 'string' && typeof bVal === 'string') {
                        return sortDirection === 'asc'
                            ? aVal.localeCompare(bVal)
                            : bVal.localeCompare(aVal);
                    }
                    
                    if (typeof aVal === 'number' && typeof bVal === 'number') {
                        return sortDirection === 'asc'
                            ? aVal - bVal
                            : bVal - aVal;
                    }
                    
                    return 0;
                });
                
                return arr;
            },
            
            getFilteredDownloads: () => {
                const { downloads, filters } = get();
                let arr = Object.values(downloads);
                
                if (filters.status?.length) {
                    arr = arr.filter(d => filters.status!.includes(d.status));
                }
                
                if (filters.category) {
                    arr = arr.filter(d => d.category === filters.category);
                }
                
                if (filters.search) {
                    const search = filters.search.toLowerCase();
                    arr = arr.filter(d => 
                        d.filename.toLowerCase().includes(search) ||
                        d.url.toLowerCase().includes(search)
                    );
                }
                
                return arr;
            },
        }),
        {
            name: 'vajra-downloads',
            partialize: (state) => ({
                sortBy: state.sortBy,
                sortDirection: state.sortDirection,
                filters: state.filters,
            }),
        }
    )
);
```

### 7.2 API Client with TypeScript

```typescript
// vajra-ui-tauri/src/api/client.ts

import type { Download, Config, VaultCredential } from './types';

const BASE_URL = 'http://127.0.0.1:6277';

class ApiError extends Error {
    constructor(public status: number, message: string) {
        super(message);
    }
}

async function request<T>(
    path: string,
    options: RequestInit = {}
): Promise<T> {
    const config = await loadConfig();
    const headers = new Headers(options.headers);
    
    if (config.auth_token) {
        headers.set('Authorization', `Bearer ${config.auth_token}`);
    }
    
    const response = await fetch(`${BASE_URL}${path}`, {
        ...options,
        headers,
    });
    
    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: response.statusText }));
        throw new ApiError(response.status, error.error?.message || 'API error');
    }
    
    return response.json();
}

export const api = {
    // Downloads
    listDownloads: () => request<Download[]>('/api/v1/downloads'),
    
    getDownload: (id: string) => request<Download>(`/api/v1/downloads/${id}`),
    
    addDownload: (req: AddDownloadRequest) => 
        request<{ id: string }>('/api/v1/downloads', {
            method: 'POST',
            body: JSON.stringify(req),
        }),
    
    updateDownload: (id: string, updates: Partial<Download>) =>
        request<Download>(`/api/v1/downloads/${id}`, {
            method: 'PATCH',
            body: JSON.stringify(updates),
        }),
    
    deleteDownload: (id: string, deleteFile = false) =>
        request<void>(`/api/v1/downloads/${id}?delete_file=${deleteFile}`, {
            method: 'DELETE',
        }),
    
    // Config
    getConfig: () => request<Config>('/api/v1/config'),
    
    updateConfig: (updates: Partial<Config>) =>
        request<Config>('/api/v1/config', {
            method: 'PATCH',
            body: JSON.stringify(updates),
        }),
    
    // Vault
    listVault: () => request<VaultCredential[]>('/api/v1/vault'),
    
    addVaultEntry: (cred: AddVaultCredentialRequest) =>
        request<VaultCredential>('/api/v1/vault', {
            method: 'POST',
            body: JSON.stringify(cred),
        }),
    
    deleteVaultEntry: (id: string) =>
        request<void>(`/api/v1/vault/${id}`, {
            method: 'DELETE',
        }),
    
    // Utilities
    inspectUrl: (url: string) =>
        request<InspectionResult>('/api/v1/inspect', {
            method: 'POST',
            body: JSON.stringify({ url }),
        }),
    
    // SSE
    connectSSE: (): EventSource => {
        return new EventSource(`${BASE_URL}/api/v1/events`);
    },
};
```

---

## 8. Distribution & Deployment Plan

### 8.1 Auto-Update Server Setup

```yaml
# GitHub Actions workflow for updates

name: Publish Update
on:
  release:
    types: [published]

jobs:
  publish-update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Generate update manifest
        run: |
          VERSION=${{ github.event.release.tag_name }}
          cat > latest.json <<EOF
          {
            "version": "${VERSION}",
            "notes": "Release notes here",
            "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
            "platforms": {
              "windows-x86_64": {
                "url": "https://github.com/user/vajra/releases/download/${VERSION}/vajra-windows.exe",
                "signature": "${{ secrets.WINDOWS_SIGNATURE }}"
              },
              "linux-x86_64": {
                "url": "https://github.com/user/vajra/releases/download/${VERSION}/vajra-linux",
                "signature": "${{ secrets.LINUX_SIGNATURE }}"
              },
              "darwin-aarch64": {
                "url": "https://github.com/user/vajra/releases/download/${VERSION}/vajra-macos-arm",
                "signature": "${{ secrets.MACOS_SIGNATURE }}"
              }
            }
          }
          EOF
      
      - name: Upload to GitHub Pages
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./
```

### 8.2 Code Signing Setup

```yaml
# .github/workflows/sign.yml

name: Sign Binaries
on:
  workflow_call:
    inputs:
      artifact-name:
        required: true
        type: string

jobs:
  sign:
    runs-on: windows-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: ${{ inputs.artifact-name }}
      
      - name: Sign with EV certificate
        env:
          CERTIFICATE: ${{ secrets.CODE_SIGNING_CERT }}
          CERTIFICATE_PASSWORD: ${{ secrets.CERT_PASSWORD }}
        run: |
          # Save certificate
          $certBytes = [Convert]::FromBase64String($env:CERTIFICATE)
          [IO.File]::WriteAllBytes("cert.pfx", $certBytes)
          
          # Sign executable
          & "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe" sign /f cert.pfx /p $env:CERTIFICATE_PASSWORD /tr http://timestamp.digicert.com /td sha256 /fd sha256 vajra.exe
          
          # Sign installer
          & "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe" sign /f cert.pfx /p $env:CERTIFICATE_PASSWORD /tr http://timestamp.digicert.com /td sha256 /fd sha256 vajra-installer.exe
      
      - uses: actions/upload-artifact@v4
        with:
          name: signed-${{ inputs.artifact-name }}
          path: |
            vajra.exe
            vajra-installer.exe
```

### 8.3 Package Manager Integration

**winget:**

```yaml
# .github/workflows/winget.yml

name: Publish to winget
on:
  release:
    types: [published]

jobs:
  winget:
    runs-on: windows-latest
    steps:
      - name: Publish to winget
        uses: vedantmgoyal2009/winget-releaser@v2
        with:
          identifier: YourName.Vajra
          installers-regex: 'vajra-windows\.exe$'
          token: ${{ secrets.GITHUB_TOKEN }}
```

**Chocolatey:**

```powershell
# tools/chocolateyInstall.ps1

$ErrorActionPreference = 'Stop'

$packageArgs = @{
    packageName   = 'vajra'
    fileType      = 'exe'
    url           = 'https://github.com/user/vajra/releases/latest/download/vajra-windows.exe'
    silentArgs    = '/S'
    checksum      = 'CHECKSUM_HERE'
    checksumType  = 'sha256'
}

Install-ChocolateyPackage @packageArgs
```

---

## 9. UX/UI Enhancement Proposals (Completed)

### 9.1 Dashboard Design

```typescript
// vajra-ui-tauri/src/components/Dashboard.tsx

import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend } from 'recharts';
import { PieChart, Pie, Cell } from 'recharts';

export function Dashboard() {
    const downloadStore = useDownloadStore();
    const stats = useStatsStore();
    
    return (
        <div className="dashboard">
            <div className="grid grid-cols-3 gap-4 mb-6">
                <StatCard 
                    title="Total Downloads" 
                    value={stats.totalDownloads}
                    trend={stats.downloadsTrend}
                />
                <StatCard 
                    title="Active Now" 
                    value={stats.activeDownloads}
                />
                <StatCard 
                    title="Speed" 
                    value={formatBytes(stats.currentSpeed) + '/s'}
                    trend={stats.speedTrend}
                />
            </div>
            
            <div className="grid grid-cols-2 gap-6">
                <div className="chart-card">
                    <h3>Download Speed (Last 24h)</h3>
                    <LineChart data={stats.speedHistory}>
                        <CartesianGrid strokeDasharray="3 3" />
                        <XAxis dataKey="time" />
                        <YAxis />
                        <Tooltip />
                        <Line type="monotone" dataKey="speed" stroke="#8884d8" />
                    </LineChart>
                </div>
                
                <div className="chart-card">
                    <h3>Downloads by Category</h3>
                    <PieChart>
                        <Pie 
                            data={stats.categoryDistribution} 
                            dataKey="count" 
                            nameKey="category"
                            cx="50%" 
                            cy="50%" 
                            outerRadius={80}
                        >
                            {stats.categoryDistribution.map((entry, index) => (
                                <Cell key={index} fill={COLORS[index % COLORS.length]} />
                            ))}
                        </Pie>
                        <Legend />
                    </PieChart>
                </div>
            </div>
            
            <div className="recent-downloads">
                <h3>Recent Downloads</h3>
                <RecentDownloadsList downloads={stats.recentDownloads} />
            </div>
        </div>
    );
}
```

### 9.2 Keyboard Shortcuts

```typescript
// vajra-ui-tauri/src/hooks/useKeyboardShortcuts.ts

import { useEffect } from 'react';
import { useDownloadStore } from '../stores/downloadStore';
import { useUIStore } from '../stores/uiStore';

export function useKeyboardShortcuts() {
    const downloadStore = useDownloadStore();
    const uiStore = useUIStore();
    
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            // Ignore if typing in input
            if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
                return;
            }
            
            // Ctrl+N: Add new download
            if (e.ctrlKey && e.key === 'n') {
                e.preventDefault();
                uiStore.openAddUrlDialog();
            }
            
            // Ctrl+F: Focus search
            if (e.ctrlKey && e.key === 'f') {
                e.preventDefault();
                document.getElementById('search-input')?.focus();
            }
            
            // Space: Pause/Resume selected
            if (e.key === ' ' && !e.ctrlKey) {
                e.preventDefault();
                const selected = Array.from(downloadStore.selectedIds);
                const downloads = selected.map(id => downloadStore.downloads[id]);
                
                const canPause = downloads.some(d => d.status === 'downloading');
                const canResume = downloads.some(d => d.status === 'paused');
                
                if (canPause) {
                    downloadStore.pauseSelected();
                } else if (canResume) {
                    downloadStore.resumeSelected();
                }
            }
            
            // Delete: Delete selected
            if (e.key === 'Delete') {
                e.preventDefault();
                if (downloadStore.selectedIds.size > 0) {
                    uiStore.openDeleteDialog();
                }
            }
            
            // Ctrl+A: Select all
            if (e.ctrlKey && e.key === 'a') {
                e.preventDefault();
                downloadStore.selectAll();
            }
            
            // Ctrl+,: Open settings
            if (e.ctrlKey && e.key === ',') {
                e.preventDefault();
                uiStore.openOptionsDialog();
            }
            
            // Alt+1-9: Switch categories
            if (e.altKey && e.key >= '1' && e.key <= '9') {
                e.preventDefault();
                const categories = ['all', 'downloading', 'paused', 'completed', 'failed'];
                const index = parseInt(e.key) - 1;
                if (index < categories.length) {
                    downloadStore.setFilters({ status: [categories[index]] });
                }
            }
        };
        
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [downloadStore, uiStore]);
}
```

### 9.3 Accessibility Improvements

```typescript
// vajra-ui-tauri/src/components/DownloadsTable.tsx - Accessibility enhancements

export function DownloadsTable() {
    return (
        <table 
            role="grid"
            aria-label="Downloads list"
            aria-rowcount={downloads.length}
        >
            <thead>
                <tr role="row">
                    <th 
                        role="columnheader"
                        aria-sort={sortBy === 'filename' ? sortDirection : 'none'}
                        onClick={() => setSortBy('filename')}
                        aria-label="Sort by filename"
                    >
                        File Name
                        {sortBy === 'filename' && (
                            <span aria-hidden="true">
                                {sortDirection === 'asc' ? '↑' : '↓'}
                            </span>
                        )}
                    </th>
                    {/* ... other columns ... */}
                </tr>
            </thead>
            <tbody>
                {downloads.map((download, index) => (
                    <tr 
                        key={download.id}
                        role="row"
                        aria-selected={selectedIds.has(download.id)}
                        aria-rowindex={index + 1}
                        tabIndex={0}
                        onKeyDown={(e) => handleRowKeyDown(e, download.id)}
                    >
                        <td role="gridcell">
                            <input 
                                type="checkbox"
                                checked={selectedIds.has(download.id)}
                                onChange={() => toggleSelection(download.id)}
                                aria-label={`Select ${download.filename}`}
                            />
                        </td>
                        <td role="gridcell">{download.filename}</td>
                        <td role="gridcell">
                            <div 
                                role="progressbar"
                                aria-valuenow={Math.round(download.bytes_done / download.total_bytes * 100)}
                                aria-valuemin={0}
                                aria-valuemax={100}
                                aria-label={`Download progress: ${Math.round(download.bytes_done / download.total_bytes * 100)}%`}
                            >
                                <div style={{ width: `${(download.bytes_done / download.total_bytes) * 100}%` }} />
                            </div>
                        </td>
                        {/* ... other cells ... */}
                    </tr>
                ))}
            </tbody>
        </table>
    );
}

// Screen reader announcements
function useScreenReaderAnnouncements() {
    const downloadStore = useDownloadStore();
    
    useEffect(() => {
        const unsubscribe = downloadStore.subscribe((state, prevState) => {
            // Detect completed downloads
            const completed = Object.values(state.downloads).filter(
                d => d.status === 'completed' && prevState.downloads[d.id]?.status !== 'completed'
            );
            
            completed.forEach(d => {
                announceToScreenReader(`${d.filename} has finished downloading`);
            });
        });
        
        return unsubscribe;
    }, []);
}

function announceToScreenReader(message: string) {
    const announcement = document.createElement('div');
    announcement.setAttribute('role', 'status');
    announcement.setAttribute('aria-live', 'polite');
    announcement.className = 'sr-only';
    announcement.textContent = message;
    document.body.appendChild(announcement);
    
    setTimeout(() => {
        document.body.removeChild(announcement);
    }, 1000);
}
```

---

## 10. Competitive Feature Parity Guide (Completed)

### 10.1 IDM Feature Parity

| IDM Feature | Vajra Status | Implementation Priority |
|---|---|---|
| Multi-source mirrors | ✅ Implemented | Done in Phase 3 |
| Auto-update | ✅ Implemented | Done in Phase 3 |
| Scheduler | ✅ Exists | — |
| Browser integration | ✅ Exists | — |
| Batch download | ✅ Implemented | Done in Phase 3 |
| Speed limiter | ✅ Exists | — |
| Video grabber | ✅ Exists | — |
| Proxy support | ✅ Exists | — |
| Auto-shutdown | ✅ Exists | — |
| Clipboard monitoring | ✅ Exists | — |
| Site spider | ✅ Exists | — |
| Category management | ✅ Implemented | Done in Phase 3 |
| Import/Export | ✅ Implemented | Done in Phase 3 |

### 10.2 JDownloader Feature Parity

| JDownloader Feature | Vajra Status | Implementation Priority |
|---|---|---|
| Captcha solving | ✅ Implemented | Done in Phase 6 |
| Link decryption (DLC, RSDF) | ✅ Implemented | Done in Phase 6 |
| Plugin system | ✅ Implemented | Done in Phase 5 |
| RSS feeds | ✅ Implemented | Done in Phase 4 |
| Account manager | ✅ Implemented | Done in Phase 6 |
| Auto-extract archives | ✅ Exists | — |
| Remote control | ✅ Implemented | Done in Phase 4 |
| Package management | ✅ Implemented | Done in Phase 4 |

---

## 11. Migration Paths

### 11.1 Database Migration Framework

```rust
// vajra-engine/src/db/migrations.rs

use rusqlite::Connection;

pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "Initial schema",
        sql: r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                request_json TEXT NOT NULL,
                state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                filename TEXT NOT NULL,
                dest_path TEXT NOT NULL,
                total_bytes INTEGER NOT NULL DEFAULT 0,
                speed_avg INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                completed_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        "#,
    },
    Migration {
        version: 2,
        description: "Add vault credentials table",
        sql: r#"
            CREATE TABLE IF NOT EXISTS vault_credentials (
                id TEXT PRIMARY KEY,
                domain TEXT NOT NULL UNIQUE,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
        "#,
    },
    Migration {
        version: 3,
        description: "Encrypt vault credentials",
        sql: r#"
            ALTER TABLE vault_credentials ADD COLUMN username_encrypted TEXT;
            ALTER TABLE vault_credentials ADD COLUMN password_encrypted TEXT;
            
            -- Migration will be done in code
        "#,
    },
    Migration {
        version: 4,
        description: "Add download categories and tags",
        sql: r#"
            ALTER TABLE jobs ADD COLUMN category TEXT;
            ALTER TABLE jobs ADD COLUMN tags TEXT;
            
            CREATE INDEX idx_jobs_category ON jobs(category);
        "#,
    },
];

pub fn run_migrations(conn: &Connection) -> Result<(), MigrationError> {
    // Get current version
    let current_version: i32 = conn.query_row(
        "SELECT value FROM settings WHERE key = 'db_version'",
        [],
        |row| row.get::<_, String>(0).and_then(|v| v.parse().map_err(Into::into))
    ).unwrap_or(0);
    
    // Run pending migrations
    for migration in MIGRATIONS {
        if migration.version > current_version {
            tracing::info!("Running migration {}: {}", migration.version, migration.description);
            
            conn.execute_batch(migration.sql)
                .map_err(|e| MigrationError::Sql(e, migration.version))?;
            
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('db_version', ?1)",
                [migration.version.to_string()],
            )?;
        }
    }
    
    Ok(())
}
```

### 11.2 State File Migration

```rust
// vajra-engine/src/state.rs

pub fn migrate_state_file(path: &Path) -> Result<DownloadState, StateError> {
    let content = std::fs::read_to_string(path)?;
    
    // Try to parse as current version
    if let Ok(state) = serde_json::from_str::<DownloadState>(&content) {
        return Ok(state);
    }
    
    // Try to parse as v1 (old format)
    if let Ok(v1) = serde_json::from_str::<DownloadStateV1>(&content) {
        tracing::info!("Migrating state file from v1 to current version");
        
        let state = DownloadState {
            id: v1.id,
            url: v1.url,
            total_bytes: v1.total_bytes,
            etag: v1.etag,
            last_modified: v1.last_modified,
            chunks: v1.chunks.into_iter().map(|c| ChunkProgress {
                chunk_id: c.id,
                bytes_written: c.bytes_done,
                start_byte: Some(c.start),
                end_byte: Some(c.end),
            }).collect(),
            paused_at: v1.timestamp,
        };
        
        // Write back in new format
        let new_content = serde_json::to_string(&state)?;
        std::fs::write(path, new_content)?;
        
        return Ok(state);
    }
    
    Err(StateError::UnsupportedVersion)
}
```

---

## 12. Performance Benchmarks & Targets

### 12.1 Benchmark Suite

```rust
// vajra-engine/benches/download_bench.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vajra_engine::{calculate_chunks, Multiplexer};

fn bench_chunk_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("chunk_calculation");
    
    for size in [1_000_000, 10_000_000, 100_000_000, 1_000_000_000u64] {
        group.bench_function(format!("{}MB", size / 1_000_000), |b| {
            b.iter(|| calculate_chunks(black_box(size), black_box(8)))
        });
    }
    
    group.finish();
}

fn bench_multiplexer_overhead(c: &mut Criterion) {
    c.bench_function("multiplexer_setup", |b| {
        b.iter(|| {
            let chunks = calculate_chunks(100_000_000, 8);
            black_box(chunks)
        })
    });
}

criterion_group!(benches, bench_chunk_calculation, bench_multiplexer_overhead);
criterion_main!(benches);
```

### 12.2 Performance Targets

| Metric | Current | Target | Measurement |
|---|---|---|---|
| Download speed (single connection) | ~90% of line speed | 95%+ | iperf3 comparison |
| Multi-threaded speedup | 2-3x | 4-6x | Single vs 8 connections |
| Memory usage (idle) | ~50MB | <30MB | Process monitor |
| Memory usage (100 downloads) | ~200MB | <150MB | Process monitor |
| Startup time | ~2s | <1s | Time to first render |
| Database query (list 1000 downloads) | ~100ms | <50ms | SQLite profiler |
| SSE event latency | ~200ms | <100ms | End-to-end measurement |
| File write throughput | ~500MB/s | ~800MB/s | SSD benchmark |

### 12.3 Load Testing

```rust
// vajra-daemon/benches/load_test.rs

use reqwest::Client;
use tokio::time::{interval, Duration};

#[tokio::test]
async fn test_concurrent_downloads() {
    let client = Client::new();
    let mut handles = Vec::new();
    
    // Start 100 concurrent downloads
    for i in 0..100 {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            client.post("http://127.0.0.1:6277/api/v1/downloads")
                .json(&serde_json::json!({
                    "url": format!("http://localhost:8080/file{}.bin", i),
                    "max_connections": 4
                }))
                .send()
                .await
        }));
    }
    
    // Wait for all to start
    for handle in handles {
        handle.await.unwrap().unwrap();
    }
    
    // Monitor progress
    let mut interval = interval(Duration::from_millis(100));
    for _ in 0..100 {
        interval.tick().await;
        
        let response = client.get("http://127.0.0.1:6277/api/v1/downloads")
            .send()
            .await
            .unwrap();
        
        let downloads: Vec<serde_json::Value> = response.json().await.unwrap();
        
        // Verify all downloads are progressing
        assert_eq!(downloads.len(), 100);
        for download in &downloads {
            let bytes_done = download["bytes_done"].as_u64().unwrap();
            assert!(bytes_done > 0 || download["status"] == "completed");
        }
    }
}
```

---

## 13. Phase 5 Implementation Guides

Phase 5 elevates Vajra from a performant download manager to an intelligent, extensible ecosystem. This section details the architectural designs for the intelligence and polish features.

### 13.1 WebAssembly Plugin System (`vajra-engine/src/plugins.rs`)

To allow community extensibility without sacrificing Rust's safety, we will use **Extism** to host WebAssembly (Wasm) plugins. Plugins can intercept requests, extract download links from complex sites, and perform custom post-processing.

```rust
// vajra-engine/Cargo.toml
// [dependencies]
// extism = "1.0"

use extism::{Plugin, Manifest, Wasm, DefaultExtismClient};
use std::collections::HashMap;

pub struct PluginManager {
    plugins: HashMap<String, Plugin<'static>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: HashMap::new() }
    }

    pub fn load_plugin(&mut self, name: &str, wasm_bytes: &[u8]) -> Result<(), extism::Error> {
        let wasm = Wasm::data(wasm_bytes);
        let manifest = Manifest::new([wasm]);
        let plugin = Plugin::new(&manifest, [], true)?;
        self.plugins.insert(name.to_string(), plugin);
        Ok(())
    }

    pub fn extract_links(&mut self, plugin_name: &str, url: &str) -> Result<Vec<String>, extism::Error> {
        if let Some(plugin) = self.plugins.get_mut(plugin_name) {
            let res = plugin.call::<&str, &str>("extract", url)?;
            // Assume the plugin returns a JSON array of URLs
            let urls: Vec<String> = serde_json::from_str(res).unwrap_or_default();
            Ok(urls)
        } else {
            Err(extism::Error::msg("Plugin not found"))
        }
    }
}
```

### 13.2 Automation Rules Engine (`vajra-engine/src/rules.rs`)

A declarative rules engine allowing users to define IF-THEN conditions for download routing and post-processing.

```rust
use regex::Regex;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutomationRule {
    pub name: String,
    pub conditions: Vec<RuleCondition>,
    pub actions: Vec<RuleAction>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RuleCondition {
    ExtensionEquals(String),
    DomainMatches(String),
    SizeGreaterThan(u64), // bytes
    FilenameRegex(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RuleAction {
    MoveToDirectory(PathBuf),
    RunScript(PathBuf),
    SetPriority(crate::queue::Priority),
    AddTag(String),
}

pub struct RulesEngine {
    pub rules: Vec<AutomationRule>,
}

impl RulesEngine {
    pub fn evaluate_and_apply(&self, req: &mut crate::config::DownloadRequest) {
        for rule in &self.rules {
            if self.matches(rule, req) {
                self.apply_actions(&rule.actions, req);
            }
        }
    }

    fn matches(&self, rule: &AutomationRule, req: &crate::config::DownloadRequest) -> bool {
        rule.conditions.iter().all(|cond| match cond {
            RuleCondition::ExtensionEquals(ext) => {
                req.url.ends_with(ext) || req.filename.as_ref().map_or(false, |f| f.ends_with(ext))
            }
            RuleCondition::DomainMatches(domain) => req.url.contains(domain),
            RuleCondition::SizeGreaterThan(_) => true, // Evaluated after metadata fetch
            RuleCondition::FilenameRegex(pattern) => {
                let re = Regex::new(pattern).unwrap();
                req.filename.as_ref().map_or(false, |f| re.is_match(f))
            }
        })
    }

    fn apply_actions(&self, actions: &[RuleAction], req: &mut crate::config::DownloadRequest) {
        for action in actions {
            match action {
                RuleAction::MoveToDirectory(dir) => {
                    req.dest_path = Some(dir.clone());
                }
                RuleAction::SetPriority(prio) => {
                    req.priority = prio.clone();
                }
                // other actions...
                _ => {}
            }
        }
    }
}
```

### 13.3 AI-Based Anomaly Detection (`vajra-engine/src/ai.rs`)

Simple statistical and heuristic models to detect stuck downloads and predict the optimal number of connections.

```rust
use std::collections::VecDeque;

pub struct AnomalyDetector {
    speed_history: VecDeque<f64>,
    max_history_size: usize,
}

impl AnomalyDetector {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            speed_history: VecDeque::with_capacity(max_history_size),
            max_history_size,
        }
    }

    pub fn record_speed(&mut self, speed_bps: f64) {
        if self.speed_history.len() == self.max_history_size {
            self.speed_history.pop_front();
        }
        self.speed_history.push_back(speed_bps);
    }

    /// Detect if the download speed has mysteriously dropped to zero
    /// despite the connection being open.
    pub fn is_stuck(&self) -> bool {
        if self.speed_history.len() < self.max_history_size {
            return false;
        }
        // If the last 5 readings are 0 but previous readings were high
        let recent_zeros = self.speed_history.iter().rev().take(5).all(|&s| s == 0.0);
        let had_good_speed = self.speed_history.iter().take(self.max_history_size - 5).any(|&s| s > 1024.0);
        
        recent_zeros && had_good_speed
    }
}
```

### 13.4 Mobile Companion App (Expo/React Native)

To allow remote management of downloads, a lightweight mobile app communicating with `vajra-daemon` over a secure WebSocket or REST API (authenticated via Bearer tokens).

```typescript
// vajra-mobile/App.tsx
import React, { useEffect, useState } from 'react';
import { View, Text, FlatList, StyleSheet } from 'react-native';

export default function App() {
  const [downloads, setDownloads] = useState([]);
  
  // Replace with actual daemon IP on the local network
  const DAEMON_URL = 'http://192.168.1.100:6277';
  const AUTH_TOKEN = 'YOUR_SECRET_TOKEN';

  useEffect(() => {
    fetch(`${DAEMON_URL}/api/v1/downloads`, {
      headers: { Authorization: `Bearer ${AUTH_TOKEN}` }
    })
      .then(res => res.json())
      .then(data => setDownloads(data))
      .catch(err => console.error("Failed to fetch downloads", err));
  }, []);

  return (
    <View style={styles.container}>
      <Text style={styles.header}>Vajra Downloads</Text>
      <FlatList
        data={downloads}
        keyExtractor={(item) => item.id}
        renderItem={({ item }) => (
          <View style={styles.item}>
            <Text style={styles.title} numberOfLines={1}>{item.filename}</Text>
            <Text>{item.status} - {Math.round(item.bytes_done / 1024 / 1024)} MB</Text>
          </View>
        )}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  container: { flex: 1, padding: 20, backgroundColor: '#fff' },
  header: { fontSize: 24, fontWeight: 'bold', marginBottom: 20 },
  item: { padding: 15, borderBottomWidth: 1, borderColor: '#eee' },
  title: { fontSize: 16, fontWeight: '500' },
});
```

---

## Conclusion

This deep dive provides concrete implementation guidance for transforming Vajra from a strong foundation into the best open-source download manager available. The key priorities are:

1. **Fix critical bugs** (Phase 0) - data corruption and security vulnerabilities
2. **Add multi-source mirrors** - table-stakes for premium download managers
3. **Implement auto-update** - without it, users never upgrade
4. **Add code signing** - Windows SmartScreen blocks unsigned installers
5. **Build comprehensive tests** - can't ship with confidence otherwise
6. **Refactor frontend** - monolithic App.tsx won't scale
7. **Add missing features** - Metalink, RSS, import/export, batch operations

With these improvements, Vajra will have:
- The performance and reliability of Rust
- The most sophisticated download engine of any open-source project
- A modern, accessible UI
- Professional distribution and update mechanisms
- Feature parity with commercial download managers

The architecture is already world-class. The implementation roadmap above will get you there.
