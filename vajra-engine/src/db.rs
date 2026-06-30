//! SQLite database for download history and application settings.
//!
//! Uses `rusqlite` with the bundled SQLite (no external dependency needed).

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};

use crate::vault;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub request_json: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultCredential {
    pub id: String,
    pub domain: String,
    pub username: String,
    pub password: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
    pub action: String,
    pub details: String,
    pub created_at: DateTime<Utc>,
}

// ─── Schema types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub dest_path: String,
    pub total_bytes: u64,
    pub speed_avg_bps: u64,
    pub status: String, // "completed" | "failed" | "cancelled"
    pub completed_at: DateTime<Utc>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    pub global_speed_limit_bps: u64, // 0 = unlimited
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub sound_on_complete: bool,
    pub dark_mode: bool,
    pub browser_integration: bool,
    pub auto_start_downloads: bool,
    pub default_connections_per_download: u32,
    pub scheduler_enabled: bool,
    pub scheduler_start_time: Option<String>,
    pub scheduler_stop_time: Option<String>,
    pub client_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        let downloads_dir = dirs_next::download_dir()
            .or_else(|| dirs_next::home_dir().map(|d| d.join("Downloads")))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .to_string_lossy()
            .to_string();

        Self {
            default_download_dir: downloads_dir,
            max_concurrent_downloads: 3,
            global_speed_limit_bps: 0,
            start_minimized: false,
            minimize_to_tray: true,
            sound_on_complete: true,
            dark_mode: true,
            browser_integration: true,
            auto_start_downloads: true,
            default_connections_per_download: 8,
            scheduler_enabled: false,
            scheduler_start_time: None,
            scheduler_stop_time: None,
            client_id: "".to_string(),
        }
    }
}

// ─── Database handle ──────────────────────────────────────────────────────────

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) the Vajra database at the given path.
    pub fn open(path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS history (
                id           TEXT PRIMARY KEY,
                url          TEXT NOT NULL,
                filename     TEXT NOT NULL,
                dest_path    TEXT NOT NULL,
                total_bytes  INTEGER NOT NULL DEFAULT 0,
                speed_avg    INTEGER NOT NULL DEFAULT 0,
                status       TEXT NOT NULL,
                completed_at TEXT NOT NULL,
                tags         TEXT NOT NULL DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS jobs (
                id           TEXT PRIMARY KEY,
                request_json TEXT NOT NULL,
                state        TEXT NOT NULL,
                created_at   TEXT NOT NULL,
                updated_at   TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS download_segments (
                job_id        TEXT NOT NULL,
                segment_id    INTEGER NOT NULL,
                start_byte    INTEGER NOT NULL,
                end_byte      INTEGER NOT NULL,
                bytes_written INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (job_id, segment_id),
                FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS job_redirects (
                job_id    TEXT PRIMARY KEY,
                final_url TEXT NOT NULL,
                FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS vault_credentials (
                id           TEXT PRIMARY KEY,
                domain       TEXT NOT NULL UNIQUE,
                username     TEXT NOT NULL,
                password     TEXT NOT NULL,
                created_at   TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_logs (
                id           TEXT PRIMARY KEY,
                action       TEXT NOT NULL,
                details      TEXT NOT NULL,
                created_at   TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_history_completed ON history(completed_at DESC);

            CREATE TABLE IF NOT EXISTS file_hashes (
                dest_path TEXT PRIMARY KEY,
                hash      TEXT NOT NULL,
                size      INTEGER NOT NULL
            );

            -- BUG-13 normalisation: rewrite any legacy 'completed' rows written by
            -- older builds to the canonical 'complete' string used by state_str().
            -- This runs on every open but is a no-op once the rows are normalised.
            UPDATE jobs SET state = 'complete' WHERE state = 'completed';
        ",
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS rss_feeds (
                id TEXT PRIMARY KEY,
                url TEXT UNIQUE NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS rss_items (
                id TEXT PRIMARY KEY,
                feed_id TEXT NOT NULL,
                guid TEXT NOT NULL,
                download_id TEXT,
                created_at TEXT NOT NULL,
                UNIQUE(feed_id, guid)
            )",
            [],
        )?;

        // Try adding the auto_rename column if it doesn't exist. Ignore error if it does.
        let _ = self.conn.execute("ALTER TABLE history ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'", []);
        Ok(())
    }

    pub fn upsert_job(&self, job: &JobRecord) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO jobs (id, request_json, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               request_json = excluded.request_json,
               state = excluded.state,
               updated_at = excluded.updated_at",
            params![
                job.id,
                job.request_json,
                job.state,
                job.created_at.to_rfc3339(),
                job.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn update_job_state(&self, id: &str, state: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE jobs SET state = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, state, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn recoverable_jobs(&self) -> SqlResult<Vec<JobRecord>> {
        // The canonical terminal state written by `schema::state_str()` is 'complete'.
        // Legacy 'completed' rows are normalised to 'complete' by the migration above,
        // so we only need to exclude the canonical strings here.
        let mut statement = self.conn.prepare(
            "SELECT id, request_json, state, created_at, updated_at
             FROM jobs WHERE state NOT IN ('complete', 'cancelled') ORDER BY created_at ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                request_json: row.get(1)?,
                state: row.get(2)?,
                created_at: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                updated_at: row.get::<_, String>(4)?.parse().unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    pub fn load_all_jobs(&self) -> SqlResult<Vec<JobRecord>> {
        let mut statement = self.conn.prepare(
            "SELECT id, request_json, state, created_at, updated_at
             FROM jobs ORDER BY created_at ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                request_json: row.get(1)?,
                state: row.get(2)?,
                created_at: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                updated_at: row.get::<_, String>(4)?.parse().unwrap_or_default(),
            })
        })?;
        rows.collect()
    }
    pub fn load_job(&self, id: &str) -> SqlResult<Option<JobRecord>> {
        let mut statement = self.conn.prepare(
            "SELECT id, request_json, state, created_at, updated_at
             FROM jobs WHERE id = ?1",
        )?;
        let mut rows = statement.query_map(params![id], |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                request_json: row.get(1)?,
                state: row.get(2)?,
                created_at: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                updated_at: row.get::<_, String>(4)?.parse().unwrap_or_default(),
            })
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    pub fn get_history_entry(&self, id: &str) -> SqlResult<Option<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, filename, dest_path, total_bytes, speed_avg, status, completed_at, tags
             FROM history WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                filename: row.get(2)?,
                dest_path: row.get(3)?,
                total_bytes: row.get::<_, i64>(4)? as u64,
                speed_avg_bps: row.get::<_, i64>(5)? as u64,
                status: row.get(6)?,
                completed_at: row
                    .get::<_, String>(7)?
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_default(),
                tags: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
            })
        })?;
        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn get_job(&self, id: &str) -> SqlResult<Option<JobRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, request_json, state, created_at, updated_at
             FROM jobs WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(JobRecord {
                id: row.get(0)?,
                request_json: row.get(1)?,
                state: row.get(2)?,
                created_at: row.get::<_, String>(3)?.parse().unwrap_or_default(),
                updated_at: row.get::<_, String>(4)?.parse().unwrap_or_default(),
            })
        })?;
        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn delete_job(&self, id: &str) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM jobs WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ── History ───────────────────────────────────────────────────────────────

    pub fn insert_history(&self, entry: &HistoryEntry) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO history
             (id, url, filename, dest_path, total_bytes, speed_avg, status, completed_at, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id,
                entry.url,
                entry.filename,
                entry.dest_path,
                entry.total_bytes as i64,
                entry.speed_avg_bps as i64,
                entry.status,
                entry.completed_at.to_rfc3339(),
                serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn get_history(&self, limit: usize, offset: usize) -> SqlResult<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, filename, dest_path, total_bytes, speed_avg, status, completed_at, tags
             FROM history ORDER BY completed_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                filename: row.get(2)?,
                dest_path: row.get(3)?,
                total_bytes: row.get::<_, i64>(4)? as u64,
                speed_avg_bps: row.get::<_, i64>(5)? as u64,
                status: row.get(6)?,
                completed_at: row
                    .get::<_, String>(7)?
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_default(),
                tags: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    pub fn delete_history_entry(&self, id: &str) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM history WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_history(&self) -> SqlResult<()> {
        self.conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn search_history(&self, query: &str) -> SqlResult<Vec<HistoryEntry>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, url, filename, dest_path, total_bytes, speed_avg, status, completed_at, tags
             FROM history WHERE filename LIKE ?1 OR url LIKE ?1 OR tags LIKE ?1
             ORDER BY completed_at DESC LIMIT 100",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                filename: row.get(2)?,
                dest_path: row.get(3)?,
                total_bytes: row.get::<_, i64>(4)? as u64,
                speed_avg_bps: row.get::<_, i64>(5)? as u64,
                status: row.get(6)?,
                completed_at: row
                    .get::<_, String>(7)?
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_default(),
                tags: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    pub fn load_settings(&self) -> SqlResult<AppSettings> {
        let mut settings = AppSettings::default();

        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        for (k, v) in rows {
            match k.as_str() {
                "default_download_dir" => settings.default_download_dir = v,
                "max_concurrent_downloads" => {
                    settings.max_concurrent_downloads = v.parse().unwrap_or(3)
                }
                "global_speed_limit_bps" => {
                    settings.global_speed_limit_bps = v.parse().unwrap_or(0)
                }
                "start_minimized" => settings.start_minimized = v == "true",
                "minimize_to_tray" => settings.minimize_to_tray = v == "true",
                "sound_on_complete" => settings.sound_on_complete = v == "true",
                "dark_mode" => settings.dark_mode = v == "true",
                "browser_integration" => settings.browser_integration = v == "true",
                "auto_start_downloads" => settings.auto_start_downloads = v == "true",
                "default_connections_per_download" => {
                    settings.default_connections_per_download = v.parse().unwrap_or(8)
                }
                "scheduler_enabled" => settings.scheduler_enabled = v == "true",
                "scheduler_start_time" => {
                    settings.scheduler_start_time = if v.is_empty() { None } else { Some(v) }
                }
                "scheduler_stop_time" => {
                    settings.scheduler_stop_time = if v.is_empty() { None } else { Some(v) }
                }
                "client_id" => settings.client_id = v,
                _ => {}
            }
        }
        if settings.client_id.is_empty() {
            settings.client_id = uuid::Uuid::new_v4().to_string();
            let _ = self.save_settings(&settings);
        }
        Ok(settings)
    }

    pub fn save_settings(&self, s: &AppSettings) -> SqlResult<()> {
        let pairs = [
            (
                "default_download_dir",
                s.default_download_dir.as_str().to_string(),
            ),
            (
                "max_concurrent_downloads",
                s.max_concurrent_downloads.to_string(),
            ),
            (
                "global_speed_limit_bps",
                s.global_speed_limit_bps.to_string(),
            ),
            ("start_minimized", s.start_minimized.to_string()),
            ("minimize_to_tray", s.minimize_to_tray.to_string()),
            ("sound_on_complete", s.sound_on_complete.to_string()),
            ("dark_mode", s.dark_mode.to_string()),
            ("browser_integration", s.browser_integration.to_string()),
            ("auto_start_downloads", s.auto_start_downloads.to_string()),
            (
                "default_connections_per_download",
                s.default_connections_per_download.to_string(),
            ),
            ("scheduler_enabled", s.scheduler_enabled.to_string()),
            (
                "scheduler_start_time",
                s.scheduler_start_time.clone().unwrap_or_default(),
            ),
            (
                "scheduler_stop_time",
                s.scheduler_stop_time.clone().unwrap_or_default(),
            ),
            (
                "client_id",
                s.client_id.clone(),
            ),
        ];

        for (k, v) in &pairs {
            self.conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }
        Ok(())
    }

    // ── Vault ───────────────────────────────────────────────────────────────

    pub fn add_credential(&self, cred: &VaultCredential) -> SqlResult<()> {
        // Load or generate vault key
        let Some(key) = vault::load_or_generate_key() else {
            // If key generation fails, store plaintext (fallback for security)
            return self.conn.execute(
                "INSERT OR REPLACE INTO vault_credentials (id, domain, username, password, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    cred.id,
                    cred.domain,
                    cred.username,
                    cred.password,
                    cred.created_at.to_rfc3339()
                ],
            ).map(|_| ());
        };

        // Encrypt username and password
        let enc_username = vault::encrypt(&key, &cred.username)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        let enc_password = vault::encrypt(&key, &cred.password)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;

        self.conn.execute(
            "INSERT OR REPLACE INTO vault_credentials (id, domain, username, password, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                cred.id,
                cred.domain,
                enc_username,
                enc_password,
                cred.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_credentials(&self) -> SqlResult<Vec<VaultCredential>> {
        // Load vault key for decryption
        let key = vault::load_or_generate_key();

        let mut stmt = self.conn.prepare(
            "SELECT id, domain, username, password, created_at
             FROM vault_credentials ORDER BY domain ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let domain: String = row.get(1)?;
            let username_enc: String = row.get(2)?;
            let password_enc: String = row.get(3)?;
            let created_at: String = row.get(4)?;

            // Decrypt if encrypted, otherwise use plaintext (migration support)
            let (username, password) = if let Some(ref k) = key {
                (
                    self.decrypt_field(k, &username_enc, "username"),
                    self.decrypt_field(k, &password_enc, "password"),
                )
            } else {
                // No key available - return as-is (fallback)
                (username_enc, password_enc)
            };

            Ok(VaultCredential {
                id,
                domain,
                username,
                password,
                created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    pub fn get_credential_by_domain(&self, domain: &str) -> SqlResult<Option<VaultCredential>> {
        // Load vault key for decryption
        let key = vault::load_or_generate_key();

        let mut stmt = self.conn.prepare(
            "SELECT id, domain, username, password, created_at
             FROM vault_credentials WHERE domain = ?1",
        )?;
        let mut rows = stmt.query_map(params![domain], |row| {
            let id: String = row.get(0)?;
            let domain: String = row.get(1)?;
            let username_enc: String = row.get(2)?;
            let password_enc: String = row.get(3)?;
            let created_at: String = row.get(4)?;

            // Decrypt if encrypted, otherwise use plaintext (migration support)
            let (username, password) = if let Some(ref k) = key {
                (
                    self.decrypt_field(k, &username_enc, "username"),
                    self.decrypt_field(k, &password_enc, "password"),
                )
            } else {
                // No key available - return as-is (fallback)
                (username_enc, password_enc)
            };

            Ok(VaultCredential {
                id,
                domain,
                username,
                password,
                created_at: created_at.parse::<DateTime<Utc>>().unwrap_or_default(),
            })
        })?;
        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn delete_credential(&self, id: &str) -> SqlResult<()> {
        self.conn
            .execute("DELETE FROM vault_credentials WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Decrypt a credential field if it looks encrypted, otherwise return as-is (migration support).
    fn decrypt_field(&self, key: &[u8; 32], value: &str, field: &str) -> String {
        if vault::looks_encrypted(value) {
            match vault::decrypt(key, value) {
                Ok(decrypted) => decrypted,
                Err(e) => {
                    tracing::warn!("Failed to decrypt vault field {}: {}", field, e);
                    value.to_string() // Return plaintext fallback
                }
            }
        } else {
            // Plaintext (legacy credential from before encryption)
            value.to_string()
        }
    }

    // ─── RSS Feed Methods ─────────────────────────────────────────────────────────

    pub fn add_rss_feed(&self, id: &str, url: &str, title: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO rss_feeds (id, url, title, created_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(url) DO UPDATE SET title = excluded.title",
            params![id, url, title, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_all_rss_feeds(&self) -> SqlResult<Vec<vajra_protocol::RssFeed>> {
        let mut statement = self.conn.prepare("SELECT id, url, title, created_at FROM rss_feeds")?;
        let rows = statement.query_map([], |row| {
            let created_str: String = row.get(3)?;
            let created_at = created_str
                .parse::<DateTime<Utc>>()
                .map(|t| t.timestamp())
                .unwrap_or(0);
            
            Ok(vajra_protocol::RssFeed {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                created_at,
            })
        })?;
        rows.collect()
    }

    pub fn delete_rss_feed(&self, id: &str) -> SqlResult<()> {
        self.conn.execute("DELETE FROM rss_feeds WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_rss_item(&self, id: &str, feed_id: &str, guid: &str, download_id: Option<&str>) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO rss_items (id, feed_id, guid, download_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, feed_id, guid, download_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn rss_item_exists(&self, feed_id: &str, guid: &str) -> SqlResult<bool> {
        let mut stmt = self.conn.prepare("SELECT 1 FROM rss_items WHERE feed_id = ?1 AND guid = ?2")?;
        let exists = stmt.exists(params![feed_id, guid])?;
        Ok(exists)
    }

    // ─── Audit Logs ───────────────────────────────────────────────────────────────

    pub fn insert_audit_log(&self, log: &AuditLog) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO audit_logs (id, action, details, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                log.id,
                log.action,
                log.details,
                log.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_audit_logs(&self, limit: usize) -> SqlResult<Vec<AuditLog>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, action, details, created_at
             FROM audit_logs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(AuditLog {
                id: row.get(0)?,
                action: row.get(1)?,
                details: row.get(2)?,
                created_at: row
                    .get::<_, String>(3)?
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    pub fn save_segment(
        &self,
        job_id: &str,
        segment_id: usize,
        start_byte: u64,
        end_byte: u64,
        bytes_written: u64,
    ) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO download_segments (job_id, segment_id, start_byte, end_byte, bytes_written)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                job_id,
                segment_id as i64,
                start_byte as i64,
                end_byte as i64,
                bytes_written as i64,
            ],
        )?;
        Ok(())
    }

    pub fn load_segments(&self, job_id: &str) -> SqlResult<Vec<crate::state::ChunkProgress>> {
        let mut stmt = self.conn.prepare(
            "SELECT segment_id, start_byte, end_byte, bytes_written
             FROM download_segments WHERE job_id = ?1 ORDER BY segment_id ASC",
        )?;
        let rows = stmt.query_map(params![job_id], |row| {
            let chunk_id: i64 = row.get(0)?;
            let start_byte: i64 = row.get(1)?;
            let end_byte: i64 = row.get(2)?;
            let bytes_written: i64 = row.get(3)?;
            Ok(crate::state::ChunkProgress {
                chunk_id: chunk_id as usize,
                bytes_written: bytes_written as u64,
                start_byte: Some(start_byte as u64),
                end_byte: Some(end_byte as u64),
            })
        })?;
        rows.collect()
    }

    pub fn delete_segments(&self, job_id: &str) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM download_segments WHERE job_id = ?1",
            params![job_id],
        )?;
        Ok(())
    }

    pub fn save_redirect(&self, job_id: &str, final_url: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO job_redirects (job_id, final_url) VALUES (?1, ?2)",
            params![job_id, final_url],
        )?;
        Ok(())
    }

    pub fn load_redirect(&self, job_id: &str) -> SqlResult<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT final_url FROM job_redirects WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;
        if let Some(row) = rows.next()? {
            let url: String = row.get(0)?;
            Ok(Some(url))
        } else {
            Ok(None)
        }
    }

    pub fn delete_redirect(&self, job_id: &str) -> SqlResult<()> {
        self.conn.execute(
            "DELETE FROM job_redirects WHERE job_id = ?1",
            params![job_id],
        )?;
        Ok(())
    }

    pub fn save_file_hash(&self, dest_path: &str, hash: &str, size: u64) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO file_hashes (dest_path, hash, size) VALUES (?1, ?2, ?3)",
            params![dest_path, hash, size as i64],
        )?;
        Ok(())
    }

    pub fn find_duplicate_file(&self, hash: &str, exclude_path: &str) -> SqlResult<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT dest_path FROM file_hashes WHERE hash = ?1 AND dest_path != ?2")?;
        let mut rows = stmt.query(params![hash, exclude_path])?;
        if let Some(row) = rows.next()? {
            let path: String = row.get(0)?;
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }
}
