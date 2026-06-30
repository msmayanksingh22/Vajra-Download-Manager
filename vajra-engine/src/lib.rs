//! Vajra Engine — Core download library
//!
//! Public API surface used by the Tauri backend:
//!
//! ```no_run
//! use vajra_engine::{DownloadManager, DownloadRequest};
//! ```

pub mod allocator;
pub mod constants;
pub mod db;
pub mod download_task;
pub mod ffmpeg;
pub mod ftp_task;
pub mod hls;
pub mod metalink;
pub mod mirror;
pub mod multiplexer;
pub mod post_processing;
pub mod queue;
pub mod state;
pub mod throttle;
pub mod vault;
pub mod writer;
pub mod ytdlp;
pub mod s3;
pub mod ai;
pub mod plugins;
pub mod rules;
pub mod ab_test;
pub mod captcha;
pub mod decryption;
pub mod content_pipeline;
pub mod cryptography;
pub mod cloud;

pub use db::Database;
pub use download_task::{DownloadError, DownloadRequest, DownloadTask, TaskState};
pub use queue::{DownloadManager, DownloadManagerHandle};
pub use state::DownloadState;
pub use throttle::{CombinedThrottle, Throttle};
pub mod torrent_task;
