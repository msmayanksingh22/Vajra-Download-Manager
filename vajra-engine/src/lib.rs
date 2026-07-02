//! Vajra Engine — Core download library
//!
//! Public API surface used by the Tauri backend:
//!
//! ```no_run
//! use vajra_engine::{DownloadManager, DownloadRequest};
//! ```

pub mod ab_test;
pub mod ai;
pub mod allocator;
pub mod captcha;
pub mod cloud;
pub mod constants;
pub mod content_pipeline;
pub mod cryptography;
pub mod db;
pub mod decryption;
pub mod download_task;
pub mod ffmpeg;
pub mod ftp_task;
pub mod hls;
pub mod metalink;
pub mod mirror;
pub mod multiplexer;
pub mod plugins;
pub mod post_processing;
pub mod queue;
pub mod rules;
pub mod s3;
pub mod state;
pub mod throttle;
pub mod vault;
pub mod writer;
pub mod ytdlp;

pub use db::Database;
pub use download_task::{DownloadError, DownloadRequest, DownloadTask, TaskState};
pub use queue::{DownloadManager, DownloadManagerHandle};
pub use state::DownloadState;
pub use throttle::{CombinedThrottle, Throttle};
pub mod torrent_task;
