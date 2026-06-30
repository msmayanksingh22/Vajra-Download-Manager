
use chrono::Utc;
use tempfile::tempdir;
use uuid::Uuid;
use vajra_engine::state::{ChunkProgress, DownloadState};

#[test]
fn test_state_save_and_load() {
    let dir = tempdir().unwrap();
    let state_file = dir.path().join("test.vajra.state");

    let state = DownloadState {
        id: Uuid::new_v4(),
        url: "http://example.com/test.zip".to_string(),
        total_bytes: 1024,
        etag: Some("W/12345".to_string()),
        last_modified: None,
        chunks: vec![
            ChunkProgress {
                chunk_id: 0,
                bytes_written: 256,
                start_byte: Some(0),
                end_byte: Some(511),
            },
            ChunkProgress {
                chunk_id: 1,
                bytes_written: 0,
                start_byte: Some(512),
                end_byte: Some(1023),
            },
        ],
        paused_at: Utc::now(),
    };

    // Save state
    state.save(&state_file).expect("Failed to save state");

    // Ensure it exists
    assert!(state_file.exists());

    // Load state
    let loaded_state = DownloadState::load(&state_file).unwrap().unwrap();

    assert_eq!(loaded_state.id, state.id);
    assert_eq!(loaded_state.url, state.url);
    assert_eq!(loaded_state.chunks.len(), 2);

    // Test get()
    let chunk0 = loaded_state.get(0).unwrap();
    assert_eq!(chunk0.bytes_written, 256);

    let chunk1 = loaded_state.get(1).unwrap();
    assert_eq!(chunk1.bytes_written, 0);

    let chunk_not_found = loaded_state.get(2);
    assert!(chunk_not_found.is_none());
}

#[test]
fn test_state_load_non_existent() {
    let dir = tempdir().unwrap();
    let state_file = dir.path().join("does_not_exist.vajra.state");

    let loaded_state = DownloadState::load(&state_file).unwrap();
    assert!(loaded_state.is_none());
}
