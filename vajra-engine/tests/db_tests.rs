use chrono::Utc;
use tempfile::tempdir;
use vajra_engine::db::{AppSettings, Database, HistoryEntry, JobRecord, VaultCredential};

fn create_temp_db() -> (tempfile::TempDir, Database) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).unwrap();
    (dir, db)
}

#[test]
fn test_db_open_and_migrate() {
    let _ = create_temp_db();
    // If it opens and migrates without panicking, the test passes.
}

#[test]
fn test_settings_crud() {
    let (_dir, db) = create_temp_db();

    // Default settings
    let settings = AppSettings {
        global_speed_limit_bps: 1000,
        dark_mode: false,
        ..Default::default()
    };

    // Save
    db.save_settings(&settings).unwrap();

    // Load
    let loaded = db.load_settings().unwrap();
    assert_eq!(loaded.global_speed_limit_bps, 1000);
    assert!(!loaded.dark_mode);
}

#[test]
fn test_jobs_crud() {
    let (_dir, db) = create_temp_db();

    let job1 = JobRecord {
        id: "job-1".to_string(),
        request_json: r#"{"url":"http://example.com"}"#.to_string(),
        state: "queued".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let job2 = JobRecord {
        id: "job-2".to_string(),
        request_json: r#"{"url":"http://example.com/2"}"#.to_string(),
        state: "downloading".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    db.upsert_job(&job1).unwrap();
    db.upsert_job(&job2).unwrap();

    let jobs = db.load_all_jobs().unwrap();
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().any(|j| j.id == "job-1"));
    assert!(jobs.iter().any(|j| j.id == "job-2"));

    db.delete_job("job-1").unwrap();
    let jobs = db.load_all_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, "job-2");
}

#[test]
fn test_history_crud() {
    let (_dir, db) = create_temp_db();

    let entry = HistoryEntry {
        id: "hist-1".to_string(),
        url: "http://example.com/file".to_string(),
        filename: "file".to_string(),
        dest_path: "/tmp/file".to_string(),
        total_bytes: 1024,
        speed_avg_bps: 512,
        status: "complete".to_string(),
        completed_at: Utc::now(),
        tags: vec![],
    };

    db.insert_history(&entry).unwrap();

    let history = db.get_history(100, 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, "hist-1");
    assert_eq!(history[0].total_bytes, 1024);

    db.clear_history().unwrap();
    let history = db.get_history(100, 0).unwrap();
    assert_eq!(history.len(), 0);
}

#[test]
fn test_vault_credentials() {
    let (_dir, db) = create_temp_db();

    // Store a credential
    let cred = VaultCredential {
        id: "cred-1".to_string(),
        domain: "example.com".to_string(),
        username: "user1".to_string(),
        password: "pass1".to_string(),
        created_at: Utc::now(),
    };
    db.add_credential(&cred).unwrap();

    // Retrieve
    let creds = db.get_credentials().unwrap();
    assert_eq!(creds.len(), 1);
    assert_eq!(creds[0].domain, "example.com");
    assert_eq!(creds[0].username, "user1");

    let cred = db.get_credential_by_domain("example.com").unwrap().unwrap();
    assert_eq!(cred.username, "user1");

    // Remove
    db.delete_credential("cred-1").unwrap();
    let creds = db.get_credentials().unwrap();
    assert_eq!(creds.len(), 0);
}
