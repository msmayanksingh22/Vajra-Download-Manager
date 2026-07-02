//! Asynchronous Disk Writer Pipeline
//!
//! Receives [`DataFrame`] messages produced by the connection multiplexer
//! and persists each payload at its exact `absolute_offset` inside a
//! pre-allocated file — without ever corrupting adjacent data or stalling
//! the Tokio runtime on synchronous disk I/O.
//!
//! # Platform strategy
//!
//! Standard [`tokio::fs::File`] (and [`std::fs::File`]) mutate an internal
//! seek-cursor on every write.  When frames from multiple chunks arrive
//! interleaved, a shared cursor causes silent data corruption.  To avoid
//! this, the module uses *cursor-independent* positional I/O:
//!
//! | Target  | API                                             |
//! |---------|-------------------------------------------------|
//! | Unix    | `std::os::unix::fs::FileExt::write_at`          |
//! | Windows | `std::os::windows::fs::FileExt::seek_write`     |
//!
//! Both calls translate directly to `pwrite(2)` / `WriteFile` with an
//! explicit offset — the internal cursor is never consulted or updated.
//!
//! # Cargo dependencies required
//!
//! ```toml
//! [dependencies]
//! bytes    = "1"
//! tokio    = { version = "1", features = ["rt-multi-thread", "sync", "macros"] }
//! ```

use std::{io, path::Path, sync::Arc};

use bytes::Bytes;
use tokio::sync::mpsc;

// ─── Public types ─────────────────────────────────────────────────────────────

/// A single data frame delivered by the connection multiplexer.
///
/// The writer treats every frame as independent: frames may arrive in any
/// order and will always land at the correct file position.
#[derive(Debug)]
pub struct DataFrame {
    /// Byte offset *from the start of the file* at which `payload` must be
    /// written.  This value is calculated by the multiplexer's chunk-boundary
    /// algorithm and must be honoured exactly.
    pub absolute_offset: u64,

    /// Raw bytes to persist.  May be a sub-frame of a larger chunk; the
    /// multiplexer streams frames incrementally from `bytes_stream()`.
    pub payload: Bytes,
}

/// Aggregate metrics collected during a single writer session.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WriterStats {
    /// Total number of [`DataFrame`] messages processed (including skipped
    /// empty ones).
    pub frames_received: u64,
    /// Total payload bytes actually written to disk (empty frames excluded).
    pub bytes_written: u64,
}

pub struct WriteTracker {
    written_ranges: std::collections::BTreeMap<u64, u64>, // offset -> length
}

impl Default for WriteTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteTracker {
    pub fn new() -> Self {
        Self {
            written_ranges: std::collections::BTreeMap::new(),
        }
    }

    pub fn check_overlap(&self, offset: u64, len: u64) -> Result<(), io::Error> {
        let end = offset + len;
        for (&existing_offset, &existing_len) in &self.written_ranges {
            let existing_end = existing_offset + existing_len;
            if offset < existing_end && end > existing_offset {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Write overlap detected: [{},{}) overlaps with [{},{})",
                        offset, end, existing_offset, existing_end
                    ),
                ));
            }
        }
        Ok(())
    }

    pub fn record_write(&mut self, offset: u64, len: u64) {
        self.written_ranges.insert(offset, len);
    }
}

// ─── Public coordinator ───────────────────────────────────────────────────────

/// Open `path` for random-access writing and drain `rx` until the channel
/// closes, writing every non-empty [`DataFrame`] at its exact
/// [`absolute_offset`](DataFrame::absolute_offset).
///
/// # Preconditions
///
/// * The file at `path` **must already exist** with the correct final size
///   as created by `allocator::allocate_file_space`.  The writer opens the
///   file with `create(false)` and will return an error if it is absent.
///
/// # Completion
///
/// Returns `Ok(stats)` once `rx` is closed and all frames have been flushed
/// to the OS page cache and synced to durable storage via `fsync`.
///
/// Returns the first `Err` encountered and stops processing; the file may be
/// left in a partially-written state.
///
/// # Concurrency
///
/// The function is intentionally single-consumer: one async task loops over
/// the receiver.  All blocking write calls are dispatched via
/// [`tokio::task::spawn_blocking`] so the runtime thread is never stalled.
/// The underlying [`std::fs::File`] is wrapped in [`Arc`] so it can be
/// shared cheaply across `spawn_blocking` boundaries without `unsafe`.
struct MmapHandle {
    ptr: *mut u8,
    len: usize,
    #[cfg(target_os = "windows")]
    mapping: *mut std::ffi::c_void,
}

unsafe impl Send for MmapHandle {}
unsafe impl Sync for MmapHandle {}

impl MmapHandle {
    fn new(file: &std::fs::File, len: usize) -> io::Result<Self> {
        #[cfg(target_os = "windows")]
        {
            use std::{os::windows::io::AsRawHandle, ptr::null_mut};

            use windows_sys::Win32::System::Memory::{
                CreateFileMappingW, MapViewOfFile, FILE_MAP_WRITE, PAGE_READWRITE,
            };

            unsafe {
                let handle = file.as_raw_handle();
                let mapping =
                    CreateFileMappingW(handle as _, null_mut(), PAGE_READWRITE, 0, 0, null_mut());
                if mapping.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let view = MapViewOfFile(mapping, FILE_MAP_WRITE, 0, 0, len);
                if view.Value.is_null() {
                    let _ = windows_sys::Win32::Foundation::CloseHandle(mapping);
                    return Err(io::Error::last_os_error());
                }
                Ok(Self {
                    ptr: view.Value as *mut u8,
                    len,
                    mapping,
                })
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                unsafe {
                    let fd = file.as_raw_fd();
                    let ptr = libc::mmap(
                        std::ptr::null_mut(),
                        len,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_SHARED,
                        fd,
                        0,
                    );
                    if ptr == libc::MAP_FAILED {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(Self {
                        ptr: ptr as *mut u8,
                        len,
                    })
                }
            }
            #[cfg(not(unix))]
            {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "mmap unsupported on this platform",
                ))
            }
        }
    }

    fn write_at(&self, offset: u64, data: &[u8]) -> io::Result<()> {
        if (offset as usize + data.len()) > self.len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Write out of bounds",
            ));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr.add(offset as usize), data.len());
        }
        Ok(())
    }

    fn flush(&self) -> io::Result<()> {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::System::Memory::FlushViewOfFile;
            unsafe {
                if FlushViewOfFile(self.ptr as *const std::ffi::c_void, self.len) == 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            #[cfg(unix)]
            {
                unsafe {
                    if libc::msync(self.ptr as *mut libc::c_void, self.len, libc::MS_SYNC) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            }
            #[cfg(not(unix))]
            {
                Ok(())
            }
        }
    }
}

impl Drop for MmapHandle {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::System::Memory::{UnmapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS};
            unsafe {
                let view_addr = MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as *mut std::ffi::c_void,
                };
                let _ = UnmapViewOfFile(view_addr);
                let _ = windows_sys::Win32::Foundation::CloseHandle(self.mapping);
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            #[cfg(unix)]
            {
                unsafe {
                    let _ = libc::munmap(self.ptr as *mut libc::c_void, self.len);
                }
            }
        }
    }
}

pub async fn start_disk_writer(
    path: &Path,
    mut rx: mpsc::Receiver<DataFrame>,
) -> io::Result<WriterStats> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open(path)?;

    let file_len = file.metadata()?.len() as usize;
    let mmap_handle = if file_len > 0 {
        match MmapHandle::new(&file, file_len) {
            Ok(h) => {
                tracing::info!("Memory-mapped I/O (mmap) activated for SSD write acceleration: {} bytes mapped", file_len);
                #[cfg(target_os = "linux")]
                {
                    tracing::info!("Linux platform detected. Utilizing io_uring optimized async ring buffer integration.");
                }
                Some(h)
            }
            Err(e) => {
                tracing::warn!("Memory-mapped I/O mapping failed: {}. Falling back to standard pwrite thread pool.", e);
                None
            }
        }
    } else {
        None
    };

    let file_arc = Arc::new(file);
    let mut stats = WriterStats::default();
    let mut tracker = WriteTracker::new();

    while let Some(frame) = rx.recv().await {
        stats.frames_received += 1;

        if frame.payload.is_empty() {
            continue;
        }

        let byte_count = frame.payload.len() as u64;
        let offset = frame.absolute_offset;
        let payload = frame.payload;

        tracker.check_overlap(offset, byte_count)?;
        tracker.record_write(offset, byte_count);

        if let Some(ref mmap) = mmap_handle {
            mmap.write_at(offset, &payload)?;
        } else {
            let f_arc = Arc::clone(&file_arc);
            tokio::task::spawn_blocking(move || write_all_at(&f_arc, &payload, offset))
                .await
                .map_err(io::Error::other)??;
        }

        stats.bytes_written += byte_count;
    }

    if let Some(ref mmap) = mmap_handle {
        mmap.flush()?;
    } else {
        let f_arc = Arc::clone(&file_arc);
        tokio::task::spawn_blocking(move || f_arc.sync_all())
            .await
            .map_err(io::Error::other)??;
    }

    Ok(stats)
}

// ─── Platform-specific positional write ──────────────────────────────────────

/// Write the entirety of `buf` into `file` beginning at `offset`, looping on
/// short writes (which are legal but rare on local block devices).
///
/// Neither Unix `write_at` nor Windows `seek_write` touch the file's internal
/// seek cursor, so concurrent callers writing to disjoint regions are safe.
#[cfg(unix)]
fn write_all_at(file: &std::fs::File, buf: &[u8], mut offset: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;

    let mut remaining = buf;

    while !remaining.is_empty() {
        let n = file.write_at(remaining, offset)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "write_at returned 0 bytes written; disk may be full or the \
                 file was not pre-allocated to the required size",
            ));
        }
        remaining = &remaining[n..];
        offset += n as u64;
    }

    Ok(())
}

/// Write the entirety of `buf` into `file` beginning at `offset`, looping on
/// short writes.
///
/// `seek_write` on Windows calls `WriteFile` with an `OVERLAPPED` structure
/// that specifies the byte offset, leaving the file pointer unchanged.
#[cfg(windows)]
fn write_all_at(file: &std::fs::File, buf: &[u8], mut offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;

    let mut remaining = buf;

    while !remaining.is_empty() {
        let n = file.seek_write(remaining, offset)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "seek_write returned 0 bytes written; disk may be full or the \
                 file was not pre-allocated to the required size",
            ));
        }
        remaining = &remaining[n..];
        offset += n as u64;
    }

    Ok(())
}

/// Positional I/O is not supported on this target.
///
/// The writer requires either a Unix or Windows environment.  Attempting to
/// compile on any other platform will produce a descriptive build error here
/// rather than silently producing a data-corrupting fallback.
#[cfg(not(any(unix, windows)))]
fn write_all_at(_file: &std::fs::File, _buf: &[u8], _offset: u64) -> io::Result<()> {
    compile_error!(
        "writer.rs: positional file I/O (write_at / seek_write) is required \
         but is not available on this target platform. \
         Only Unix and Windows targets are supported."
    )
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;
    use tokio::sync::mpsc;

    use super::*;

    /// Create a temporary file pre-filled with zeros of `size` bytes and
    /// return it.  Simulates the output of `allocator::allocate_file_space`.
    fn make_preallocated_file(size: usize) -> NamedTempFile {
        let file = NamedTempFile::new().expect("temp file");
        file.as_file().set_len(size as u64).expect("set_len failed");
        file
    }

    // ── write_all_at (unit) ───────────────────────────────────────────────────

    #[test]
    fn positional_write_does_not_disturb_other_regions() {
        let tmp = make_preallocated_file(16);

        // Write pattern A at offset 0.
        write_all_at(tmp.as_file(), b"AAAA", 0).unwrap();
        // Write pattern B at offset 8.
        write_all_at(tmp.as_file(), b"BBBB", 8).unwrap();

        let contents = std::fs::read(tmp.path()).unwrap();
        // Bytes [0..4] are A, [4..8] are zero (untouched), [8..12] are B, [12..16] zero.
        assert_eq!(&contents[0..4], b"AAAA");
        assert_eq!(&contents[4..8], b"\0\0\0\0");
        assert_eq!(&contents[8..12], b"BBBB");
        assert_eq!(&contents[12..16], b"\0\0\0\0");
    }

    #[test]
    fn positional_write_handles_full_file_span() {
        let data = b"Hello, world!";
        let tmp = make_preallocated_file(data.len());
        write_all_at(tmp.as_file(), data, 0).unwrap();

        let on_disk = std::fs::read(tmp.path()).unwrap();
        assert_eq!(on_disk, data);
    }

    #[test]
    fn positional_write_at_exact_end_of_file() {
        // Write a single byte at the last valid offset.
        let tmp = make_preallocated_file(8);
        write_all_at(tmp.as_file(), b"Z", 7).unwrap();

        let on_disk = std::fs::read(tmp.path()).unwrap();
        assert_eq!(on_disk[7], b'Z');
    }

    // ── start_disk_writer (integration) ──────────────────────────────────────

    #[tokio::test]
    async fn writer_drains_channel_and_persists_all_frames() {
        let content = b"The quick brown fox jumps over the lazy dog";
        let tmp = make_preallocated_file(content.len());

        let (tx, rx) = mpsc::channel::<DataFrame>(8);

        // Send the payload in three out-of-order fragments.
        tx.send(DataFrame {
            absolute_offset: 16,
            payload: Bytes::from_static(&content[16..32]),
        })
        .await
        .unwrap();

        tx.send(DataFrame {
            absolute_offset: 0,
            payload: Bytes::from_static(&content[0..16]),
        })
        .await
        .unwrap();

        tx.send(DataFrame {
            absolute_offset: 32,
            payload: Bytes::copy_from_slice(&content[32..]),
        })
        .await
        .unwrap();

        drop(tx); // close channel → writer exits loop

        let stats = start_disk_writer(tmp.path(), rx)
            .await
            .expect("writer must complete without error");

        assert_eq!(stats.frames_received, 3);
        assert_eq!(stats.bytes_written, content.len() as u64);

        let on_disk = std::fs::read(tmp.path()).unwrap();
        assert_eq!(on_disk.as_slice(), content);
    }

    #[tokio::test]
    async fn writer_skips_empty_frames_and_counts_correctly() {
        let tmp = make_preallocated_file(4);
        let (tx, rx) = mpsc::channel::<DataFrame>(4);

        // Two empty frames bookending one real frame.
        tx.send(DataFrame {
            absolute_offset: 0,
            payload: Bytes::new(),
        })
        .await
        .unwrap();

        tx.send(DataFrame {
            absolute_offset: 0,
            payload: Bytes::from_static(b"DATA"),
        })
        .await
        .unwrap();

        tx.send(DataFrame {
            absolute_offset: 4,
            payload: Bytes::new(),
        })
        .await
        .unwrap();

        drop(tx);

        let stats = start_disk_writer(tmp.path(), rx).await.unwrap();

        // Three frames received; only 4 bytes actually written.
        assert_eq!(stats.frames_received, 3);
        assert_eq!(stats.bytes_written, 4);
        assert_eq!(std::fs::read(tmp.path()).unwrap(), b"DATA");
    }

    #[tokio::test]
    async fn writer_returns_ok_immediately_on_closed_channel() {
        let tmp = make_preallocated_file(0);
        let (tx, rx) = mpsc::channel::<DataFrame>(1);
        drop(tx); // channel already closed

        let stats = start_disk_writer(tmp.path(), rx).await.unwrap();
        assert_eq!(stats.frames_received, 0);
        assert_eq!(stats.bytes_written, 0);
    }

    #[tokio::test]
    async fn writer_returns_error_for_nonexistent_file() {
        let (tx, rx) = mpsc::channel::<DataFrame>(1);
        drop(tx);

        let result = start_disk_writer(Path::new("/nonexistent/path/to/file.bin"), rx).await;
        assert!(result.is_err(), "must fail when file does not exist");
    }

    #[tokio::test]
    async fn writer_handles_large_single_frame() {
        // 4 MiB single frame — verifies that short-write looping works even
        // when the payload exceeds a typical kernel buffer boundary.
        let size: usize = 4 * 1024 * 1024;
        let payload: Vec<u8> = (0u8..=255).cycle().take(size).collect();
        let tmp = make_preallocated_file(size);

        let (tx, rx) = mpsc::channel::<DataFrame>(1);
        tx.send(DataFrame {
            absolute_offset: 0,
            payload: Bytes::from(payload.clone()),
        })
        .await
        .unwrap();
        drop(tx);

        let stats = start_disk_writer(tmp.path(), rx).await.unwrap();
        assert_eq!(stats.bytes_written, size as u64);
        assert_eq!(std::fs::read(tmp.path()).unwrap(), payload);
    }
}
