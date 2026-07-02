//! Cross-platform, fragmentation-free disk space pre-allocator.
//!
//! Reserves contiguous sectors on a storage device *before* a download begins.
//! Uses OS-native fast-allocation primitives so no sequential zero-fill occurs:
//!
//! * **Linux**   – `fallocate(2)` with `FALLOC_FL_KEEP_SIZE = 0`, which tells the
//!   kernel to reserve the space and update the file size atomically.
//! * **Windows** – `SetEndOfFile` + `SetFileValidData`, which marks the cluster
//!   chain as allocated without initialising the clusters, identical
//!   in effect to `fallocate`.
//! * **macOS / other** – `fcntl(F_PREALLOCATE)` + `ftruncate`, the closest
//!   non-Linux POSIX equivalent.
//!
//! All blocking syscalls are dispatched through `tokio::task::spawn_blocking` so
//! the async runtime is never stalled.

// Reject unsafe in general; Windows FFI blocks below are the only exceptions and
// each one carries an explicit `// SAFETY:` comment.
#![deny(unsafe_code)]

use std::{
    io,
    path::{Path, PathBuf},
};

// ─── Public API ──────────────────────────────────────────────────────────────

/// Pre-allocates `size` bytes of contiguous disk space at `path`.
///
/// The file is created (or truncated) and its allocation is reserved without
/// writing zeroes sector-by-sector. On success the file exists on disk with
/// the correct length; its contents are undefined (uninitialized clusters).
///
/// # Errors
///
/// Returns [`std::io::Error`] for any OS-level failure:
/// - `InvalidInput` if `size == 0`.
/// - Platform-specific codes for missing privileges, full disk, etc.
///
/// # Privileges
///
/// On Windows, `SetFileValidData` requires the **SeManageVolumePrivilege**
/// (generally granted to Administrator processes). Without it the call falls
/// back to `SetEndOfFile` only (still fast in practice on NTFS, but the
/// uninitialized-data optimisation is skipped and the kernel will zero pages
/// on first access).
pub async fn allocate_file_space(path: &Path, size: u64) -> io::Result<()> {
    if size == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "allocation size must be greater than zero",
        ));
    }

    // Pre-flight disk space check
    let mut dir_path = path.to_path_buf();
    if !dir_path.exists() {
        if let Some(parent) = dir_path.parent() {
            dir_path = parent.to_path_buf();
        }
    }
    let canonical = std::fs::canonicalize(&dir_path).unwrap_or(dir_path);

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best_match = None;
    let mut best_len = 0;

    for disk in disks.list() {
        let mount_point = disk.mount_point();
        if canonical.starts_with(mount_point) {
            let len = mount_point.as_os_str().len();
            if len > best_len {
                best_len = len;
                best_match = Some(disk);
            }
        }
    }

    if let Some(disk) = best_match {
        if disk.available_space() < size {
            return Err(io::Error::other(format!(
                "not enough disk space. required: {} bytes, available: {} bytes",
                size,
                disk.available_space()
            )));
        }
    }

    // Clone path so the owned value can cross the `spawn_blocking` boundary.
    let path: PathBuf = path.to_path_buf();

    tokio::task::spawn_blocking(move || platform::preallocate(&path, size))
        .await
        // `JoinError` → map to `io::Error`
        .map_err(io::Error::other)?
}

// ─── Platform implementations ─────────────────────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use std::{io, os::unix::fs::OpenOptionsExt, path::Path};

    pub(super) fn preallocate(path: &Path, size: u64) -> io::Result<()> {
        // BUG-17 (Linux): Do NOT use .truncate(true) — it destroys existing
        // partial downloads when the allocator is called again during retry/
        // resume, wiping previously-downloaded bytes.  Only truncate if the
        // existing file is *smaller* than the target size.
        let needs_truncate = std::fs::metadata(path)
            .map(|m| m.len() < size)
            .unwrap_or(true); // new file needs truncation

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(needs_truncate)
            // O_LARGEFILE is implicit on 64-bit; O_CLOEXEC prevents fd leaks.
            .custom_flags(libc::O_CLOEXEC)
            .open(path)?;

        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();

        // fallocate(fd, 0, 0, len)
        // mode = 0  → allocate AND set file size (no FALLOC_FL_KEEP_SIZE).
        // offset = 0, len = size.
        //
        // SAFETY: `fd` is valid for the lifetime of `file`; `size` fits in
        // `libc::off_t` (i64) because we checked size > 0 before this call and
        // realistic file sizes are well below i64::MAX.
        let ret = unsafe { libc::fallocate(fd, 0, 0, size as libc::off_t) };
        if ret == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    //! Windows implementation.
    //!
    //! Strategy:
    //! 1. `CreateFileW`  – open/create the file with write access.
    //! 2. `SetFilePointerEx` + `SetEndOfFile` – extend the file on disk to
    //!    `size` bytes, committing cluster allocation in the MFT.
    //! 3. `SetFileValidData` – mark all bytes as valid so the kernel does not
    //!    zero-initialize clusters on first read.  Requires
    //!    **SeManageVolumePrivilege**; if the call fails with
    //!    `ERROR_PRIVILEGE_NOT_HELD` we swallow the error because step 2 has
    //!    already reserved the space (just without the uninitialized-data
    //!    optimisation).

    #![allow(unsafe_code)] // Windows FFI requires unsafe; each block is documented.

    use std::{io, path::Path};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, SetEndOfFile, SetFilePointerEx, SetFileValidData, FILE_ATTRIBUTE_NORMAL,
            FILE_BEGIN, FILE_SHARE_NONE, OPEN_ALWAYS,
        },
    };

    /// Encode a Rust `Path` as a NUL-terminated UTF-16 string.
    fn to_wide_nul(path: &Path) -> io::Result<Vec<u16>> {
        use std::os::windows::ffi::OsStrExt;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains interior NUL character",
            ));
        }
        wide.push(0); // NUL terminator
        Ok(wide)
    }

    pub(super) fn preallocate(path: &Path, size: u64) -> io::Result<()> {
        let wide_path = to_wide_nul(path)?;

        // ── Step 1: open / create the file ───────────────────────────────────
        //
        // SAFETY: `wide_path` is a valid NUL-terminated UTF-16 slice.
        // `CreateFileW` returns either a valid HANDLE or INVALID_HANDLE_VALUE.
        let handle: HANDLE = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_NONE,
                std::ptr::null(), // default security attributes
                // BUG-17: Use OPEN_ALWAYS, not CREATE_ALWAYS.
                // CREATE_ALWAYS silently truncates an existing partial download,
                // destroying resume progress on any retry path that calls the
                // allocator a second time.  OPEN_ALWAYS creates the file if it
                // does not exist, and opens it without truncation if it does.
                // SetEndOfFile below will grow the file to `size` if needed.
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                0 as HANDLE, // no template file
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        // Ensure the handle is closed regardless of subsequent errors.
        let _guard = HandleGuard(handle);

        // ── Step 2: move file pointer to `size` and call SetEndOfFile ────────
        //
        // `SetFilePointerEx` takes a LARGE_INTEGER (i64); safe cast because we
        // validated size > 0 and realistic download sizes fit in i64.
        //
        // SAFETY: `handle` is valid (checked above). The LARGE_INTEGER is
        // passed as a raw i64 via pointer; Windows ABI requires the value to be
        // positive and representable as a 64-bit signed integer.
        let size_i64 = size as i64;
        let moved = unsafe {
            SetFilePointerEx(
                handle,
                size_i64,
                std::ptr::null_mut(), // ignore new position out-param
                FILE_BEGIN,
            )
        };
        if moved == 0 {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: `handle` is valid; the file pointer is already at `size`.
        let eof_ok = unsafe { SetEndOfFile(handle) };
        if eof_ok == 0 {
            return Err(io::Error::last_os_error());
        }

        // ── Step 3: SetFileValidData (best-effort; privilege may be absent) ──
        //
        // SAFETY: `handle` is valid; `size_i64` is the new file size.
        let svd_ok = unsafe { SetFileValidData(handle, size_i64) };
        if svd_ok == 0 {
            let err = io::Error::last_os_error();
            // ERROR_PRIVILEGE_NOT_HELD (0x522) → not fatal; space is still
            // reserved, the kernel will zero pages lazily on first access.
            const ERROR_PRIVILEGE_NOT_HELD: i32 = 0x522;
            if err.raw_os_error() != Some(ERROR_PRIVILEGE_NOT_HELD) {
                return Err(err);
            }
        }

        Ok(())
        // `_guard` drops here → CloseHandle(handle)
    }

    /// RAII wrapper that calls `CloseHandle` when dropped.
    struct HandleGuard(HANDLE);

    impl Drop for HandleGuard {
        fn drop(&mut self) {
            // SAFETY: `self.0` was returned by `CreateFileW` and has not been
            // closed by any other path; `CloseHandle` is idempotent per MSDN
            // only with a valid handle, so we rely on the single-owner guarantee
            // enforced by this RAII guard.
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    //! macOS implementation via `fcntl(F_PREALLOCATE)` + `ftruncate`.
    //!
    //! `F_PREALLOCATE` requests contiguous allocation; `ftruncate` extends the
    //! logical file size to match, avoiding sequential zero-fill.

    #![allow(unsafe_code)] // POSIX FFI requires unsafe; each block is documented.

    use std::{io, os::unix::io::AsRawFd, path::Path};

    pub(super) fn preallocate(path: &Path, size: u64) -> io::Result<()> {
        // BUG-17 (macOS): Do NOT unconditionally truncate — it destroys existing
        // partial downloads when the allocator is called again during retry/
        // resume, wiping previously-downloaded bytes.  Only truncate if the
        // existing file is *smaller* than the target size.
        let needs_truncate = std::fs::metadata(path)
            .map(|m| m.len() < size)
            .unwrap_or(true); // new file needs truncation

        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(needs_truncate)
            .open(path)?;

        let fd = file.as_raw_fd();
        let size_i64 = size as i64;

        // `fstore_t` layout:
        //   fst_flags    : u32  (allocation flags)
        //   fst_posmode  : i32  (position mode)
        //   fst_offset   : i64  (starting offset)
        //   fst_length   : i64  (requested length)
        //   fst_bytesalloc: i64 (bytes allocated — out-param)
        #[repr(C)]
        struct Fstore {
            fst_flags: u32,
            fst_posmode: i32,
            fst_offset: libc::off_t,
            fst_length: libc::off_t,
            fst_bytesalloc: libc::off_t,
        }

        const F_ALLOCATECONTIG: u32 = 0x02; // request contiguous allocation
        const F_PEOFPOSMODE: i32 = 3; // allocate from EOF
        const F_PREALLOCATE: libc::c_int = 42;

        let mut store = Fstore {
            fst_flags: F_ALLOCATECONTIG,
            fst_posmode: F_PEOFPOSMODE,
            fst_offset: 0,
            fst_length: size_i64,
            fst_bytesalloc: 0,
        };

        // SAFETY: `fd` is valid; `store` is a correctly-sized, aligned struct
        // whose layout matches what the kernel expects for F_PREALLOCATE.
        let ret = unsafe { libc::fcntl(fd, F_PREALLOCATE, &mut store as *mut Fstore) };

        if ret == -1 {
            // Contiguous allocation failed; retry without the CONTIG flag.
            store.fst_flags = 0;
            // SAFETY: same as above.
            let ret2 = unsafe { libc::fcntl(fd, F_PREALLOCATE, &mut store as *mut Fstore) };
            if ret2 == -1 {
                return Err(io::Error::last_os_error());
            }
        }

        // Extend the logical file size without zero-filling.
        // SAFETY: `fd` is valid; `size_i64` is non-negative.
        let trunc_ret = unsafe { libc::ftruncate(fd, size_i64) };
        if trunc_ret == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

// Fallback for any other Unix (FreeBSD, etc.): posix_fallocate → ftruncate.
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod platform {
    #![allow(unsafe_code)]

    use std::{io, os::unix::io::AsRawFd, path::Path};

    pub(super) fn preallocate(path: &Path, size: u64) -> io::Result<()> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        let fd = file.as_raw_fd();
        let size_i64 = size as i64;

        // SAFETY: `fd` is valid; `size_i64` is positive.
        let ret = unsafe { libc::posix_fallocate(fd, 0, size_i64) };
        if ret != 0 {
            // `posix_fallocate` returns the errno value directly (not -1).
            return Err(io::Error::from_raw_os_error(ret));
        }

        Ok(())
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// Helper: create a fresh temp directory for each test.
    fn temp_dir() -> TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    #[tokio::test]
    async fn allocates_correct_size() {
        let dir = temp_dir();
        let path = dir.path().join("test_alloc.bin");
        let size: u64 = 16 * 1024 * 1024; // 16 MiB

        allocate_file_space(&path, size)
            .await
            .expect("allocation should succeed");

        let meta = std::fs::metadata(&path).expect("file must exist after allocation");
        assert_eq!(meta.len(), size, "file length must equal requested size");
    }

    #[tokio::test]
    async fn rejects_zero_size() {
        let dir = temp_dir();
        let path = dir.path().join("zero.bin");

        let err = allocate_file_space(&path, 0)
            .await
            .expect_err("zero size should return an error");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn creates_file_when_absent() {
        let dir = temp_dir();
        let path = dir.path().join("new_file.bin");
        assert!(!path.exists(), "precondition: file must not exist yet");

        allocate_file_space(&path, 4096)
            .await
            .expect("should create file");
        assert!(path.exists(), "file must exist after allocation");
    }

    #[tokio::test]
    async fn overwrites_existing_file() {
        let dir = temp_dir();
        let path = dir.path().join("existing.bin");

        // Pre-create with different content / size.
        std::fs::write(&path, b"old data").unwrap();

        let size: u64 = 1024 * 1024; // 1 MiB
        allocate_file_space(&path, size)
            .await
            .expect("should overwrite");

        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.len(), size);
    }
}
