use std::{
    process::{Command, Stdio},
    time::Duration,
};

struct ChildGuard(Option<std::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[tokio::test]
async fn test_api_security_and_hardening_integration() {
    let binary = env!("CARGO_BIN_EXE_vajrad");
    let temp_dir = tempfile::tempdir().unwrap();
    let test_port = 16277;

    // We start the daemon with custom port and isolated data directory
    let child = Command::new(binary)
        .env("VAJRA_PORT", test_port.to_string())
        .env("VAJRA_DATA_DIR", temp_dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn vajrad");

    let mut guard = ChildGuard(Some(child));

    // Give it a moment to bind and bootstrap
    tokio::time::sleep(Duration::from_millis(1500)).await;

    if let Some(ref mut c) = guard.0 {
        if let Some(status) = c.try_wait().unwrap() {
            panic!("vajrad exited prematurely with status: {status}");
        }
    }

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{}", test_port);

    // 1. VAJRA_PORT & Health check is unauthenticated
    let resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .expect("Failed to reach health endpoint on VAJRA_PORT");
    assert!(resp.status().is_success());

    // 2. Unauthenticated Config GET returns 401 Unauthorized with structured JSON error
    let resp = client
        .get(format!("{}/api/v1/config", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);
    let err_json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(err_json["error"]["code"], "unauthorized");

    // 3. Read bootstrapped token from isolated temp_dir
    let token_path = temp_dir.path().join("api.token");
    assert!(
        token_path.exists(),
        "api.token must be generated on first run"
    );
    let token = std::fs::read_to_string(&token_path)
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(
        token.len(),
        64,
        "Generated token must be 32 bytes hex (64 chars)"
    );

    // 4. Query token on mutation/REST endpoint MUST be rejected (401)
    let resp = client
        .get(format!("{}/api/v1/config?token={}", base_url, token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

    // 5. Authenticated Config GET with Bearer token succeeds
    let resp = client
        .get(format!("{}/api/v1/config", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let cfg: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        cfg["api_token"], "********",
        "api_token must be masked on export/GET"
    );

    // 6. WebDAV requires authentication AND is disabled by default (returns 404 when disabled)
    let resp = client
        .get(format!("{}/webdav/test.txt", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "WebDAV must require authentication"
    );

    let resp = client
        .get(format!("{}/webdav/test.txt", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::NOT_FOUND,
        "WebDAV must return 404 when disabled by default"
    );

    // 7. Add download with traversal filename and secret credentials
    let add_body = serde_json::json!({
        "url": "https://secretuser:secretpass@example.com/files/archive.zip",
        "filename": "../../evil.bat",
        "cookie_header": "sensitive_session=secret123",
        "authorization": "Bearer secret-bearer-token",
        "output_dir": temp_dir.path().to_string_lossy()
    });

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&add_body)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let add_res: serde_json::Value = resp.json().await.unwrap();
    let download_id = add_res["id"].as_str().unwrap();

    // 8. Verify filename traversal was sanitized
    let resp = client
        .get(format!("{}/api/v1/downloads/{}", base_url, download_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let dl_info: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        dl_info["filename"], "evil.bat",
        "Path traversal in filename must be stripped"
    );

    // 9. Preview executable rejection (reject .bat file preview)
    let evil_file = temp_dir.path().join("evil.bat");
    std::fs::write(&evil_file, b"@echo off\r\ncalc.exe").unwrap();

    let resp = client
        .post(format!(
            "{}/api/v1/downloads/{}/preview",
            base_url, download_id
        ))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let err: serde_json::Value = resp.json().await.unwrap();
    let msg = err["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("strictly prohibited"),
        "Expected prohibited message but got: '{}', dl_info: {:?}",
        msg,
        dl_info
    );

    // 10. Verify SQLite request_json has ALL secrets redacted
    let db_file = temp_dir.path().join("vajra.db");
    assert!(db_file.exists(), "vajra.db must exist in isolated data dir");
    let db = rusqlite::Connection::open(&db_file).unwrap();
    let req_json_str: String = db
        .query_row(
            "SELECT request_json FROM jobs WHERE id = ?1",
            rusqlite::params![download_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        !req_json_str.contains("secretpass"),
        "Password must not exist in SQLite request_json"
    );
    assert!(
        !req_json_str.contains("secretuser"),
        "Userinfo must not exist in SQLite request_json"
    );
    assert!(
        !req_json_str.contains("secret123"),
        "Cookies must not exist in SQLite request_json"
    );
    assert!(
        !req_json_str.contains("secret-bearer-token"),
        "Bearer token must not exist in SQLite request_json"
    );

    // 11. Rate limiting on expensive endpoints (inspect)
    let inspect_url = format!("{}/api/v1/inspect", base_url);
    let mut futs = Vec::new();
    for _ in 0..35 {
        futs.push(
            client
                .post(&inspect_url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&serde_json::json!({ "url": "http://127.0.0.1:1/test" }))
                .send(),
        );
    }
    let responses = futures_util::future::join_all(futs).await;
    let rate_limited = responses.iter().any(|r| {
        r.as_ref()
            .map(|resp| resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS)
            .unwrap_or(false)
    });
    assert!(
        rate_limited,
        "Hammering /inspect endpoint concurrently must trigger rate limiter (429)"
    );

    // 12. Config import security: cannot disable token or enable writable WebDAV
    let mut import_cfg = cfg.clone();
    import_cfg["api_token"] = serde_json::json!(""); // Attempt to clear token
    import_cfg["webdav_enabled"] = serde_json::json!(true);
    import_cfg["webdav_read_only"] = serde_json::json!(false); // Attempt writable WebDAV

    let resp = client
        .post(format!("{}/api/v1/config/import", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&import_cfg)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    // Verify token was NOT disabled / cleared
    let resp = client
        .get(format!("{}/api/v1/config", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let verify_cfg: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        verify_cfg["api_token"], "********",
        "Token must remain active after import"
    );
    assert_eq!(
        verify_cfg["webdav_read_only"], true,
        "WebDAV on import must be forced read-only"
    );

    // 13. [B1 Regression] Per-request output_dir validation in add_download
    // a) System32 path rejected
    let resp = client
        .post(format!("{}/api/v1/downloads", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "url": "https://example.com/test.zip",
            "output_dir": "C:\\Windows\\System32"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "System32 output_dir must be rejected"
    );

    // b) Traversal path rejected
    let resp = client
        .post(format!("{}/api/v1/downloads", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "url": "https://example.com/test.zip",
            "output_dir": "../../evil"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "Traversal output_dir must be rejected"
    );

    // c) Windows Startup directory rejected
    let resp = client
        .post(format!("{}/api/v1/downloads", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "url": "https://example.com/test.zip",
            "output_dir": "C:\\Users\\Default\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "Startup output_dir must be rejected"
    );

    // 14. [S3 Regression] "********" placeholder token cannot become active API token
    let mut patch_cfg = cfg.clone();
    patch_cfg["api_token"] = serde_json::json!("********");
    let resp = client
        .patch(format!("{}/api/v1/config", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&patch_cfg)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    // Attempting to authenticate with "********" MUST be rejected (401)
    let resp = client
        .get(format!("{}/api/v1/config", base_url))
        .header("Authorization", "Bearer ********")
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "'********' must never be accepted as a valid bearer token"
    );

    // Existing active token MUST still work
    let resp = client
        .get(format!("{}/api/v1/config", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Original token must still be valid"
    );

    // 15. [B3 Regression] WebDAV root pivoting prevention
    // a) Enable WebDAV via patch
    let mut webdav_cfg = cfg.clone();
    webdav_cfg["webdav_enabled"] = serde_json::json!(true);
    let resp = client
        .patch(format!("{}/api/v1/config", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&webdav_cfg)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    // b) Attempting to change default_output_dir while WebDAV is enabled must be rejected
    let mut pivot_cfg = webdav_cfg.clone();
    pivot_cfg["default_output_dir"] = serde_json::json!("C:\\Windows");
    let resp = client
        .patch(format!("{}/api/v1/config", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&pivot_cfg)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "Cannot change default_output_dir while WebDAV is enabled"
    );

    // c) Dedicated webdav_shared directory is created and served
    let resp = client
        .get(format!("{}/webdav/nonexistent.txt", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);

    // 16. [B2 Regression] First-party client authentication verification
    // Verify unauthenticated access to /api/v1/intercept is 401
    let resp = client
        .post(format!("{}/api/v1/intercept", base_url))
        .json(&serde_json::json!({
            "url": "https://example.com/file.zip"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "Unauthenticated intercept must return 401"
    );

    // Authenticated intercept with bearer token succeeds
    let resp = client
        .post(format!("{}/api/v1/intercept", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "url": "https://example.com/file.zip"
        }))
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "Authenticated intercept must succeed"
    );

    // 17. [Spider Authentication Verification]
    // a) Unauthenticated GET /api/v1/spider returns 401
    let resp = client
        .get(format!(
            "{}/api/v1/spider?url=http://127.0.0.1:1/test",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "Unauthenticated spider access must return 401"
    );

    // b) GET /api/v1/spider with invalid query token returns 401
    let resp = client
        .get(format!(
            "{}/api/v1/spider?url=http://127.0.0.1:1/test&token=invalid-token-12345678",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "Spider with invalid query token must return 401"
    );

    // c) Query token on non-streaming endpoint (/api/v1/downloads) MUST be rejected with 401
    let resp = client
        .get(format!("{}/api/v1/downloads?token={}", base_url, token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "Query-token on non-streaming /api/v1/downloads must be rejected with 401"
    );

    // d) Authenticated GET /api/v1/spider with valid query token succeeds with SSE stream
    let resp = client
        .get(format!(
            "{}/api/v1/spider?url=http://127.0.0.1:1/test&token={}",
            base_url, token
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "Authenticated spider with valid query token must succeed (200 OK)"
    );
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/event-stream"),
        "Spider response must be an SSE stream, got: {}",
        content_type
    );

    // 10. Bulk Action API test
    // a) Rejection: batch size >500 submitted IDs before deduplication (400 Bad Request)
    let ids_501: Vec<String> = (0..501).map(|_| uuid::Uuid::new_v4().to_string()).collect();
    let resp_501 = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "ids": ids_501,
            "action": "pause"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_501.status(), reqwest::StatusCode::BAD_REQUEST);

    // b) Rejection: all: true combined with IDs (400 Bad Request)
    let resp_all_and_ids = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "all": true,
            "ids": [uuid::Uuid::new_v4().to_string()],
            "action": "pause"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_all_and_ids.status(), reqwest::StatusCode::BAD_REQUEST);

    // c) Rejection: all: false with empty IDs (400 Bad Request)
    let resp_empty = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "all": false,
            "ids": [],
            "action": "pause"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp_empty.status(), reqwest::StatusCode::BAD_REQUEST);

    // d) Add real test download
    let add_resp = client
        .post(format!("{}/api/v1/downloads", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "url": "https://example.com/bulk_test_file.bin"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(add_resp.status(), reqwest::StatusCode::CREATED);
    let add_json: serde_json::Value = add_resp.json().await.unwrap();
    let download_id = add_json["id"].as_str().unwrap().to_string();

    // e) Deduplication test: identical IDs in request produce 1 result
    let dup_resp = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "ids": [download_id.clone(), download_id.clone()],
            "action": "pause"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(dup_resp.status(), reqwest::StatusCode::OK);
    let dup_json: serde_json::Value = dup_resp.json().await.unwrap();
    assert_eq!(
        dup_json["total"], 1,
        "Duplicate IDs must be deduplicated to 1 item"
    );

    // f) Invalid retry test: retry on active/queued download is rejected with invalid_state
    let retry_resp = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "ids": [download_id.clone()],
            "action": "retry"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(retry_resp.status(), reqwest::StatusCode::OK);
    let retry_json: serde_json::Value = retry_resp.json().await.unwrap();
    assert_eq!(retry_json["failed"][0]["id"], download_id);
    assert_eq!(retry_json["failed"][0]["code"], "invalid_state");

    // g) Bulk resume by ID
    let bulk_resume_resp = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "ids": [download_id.clone()],
            "action": "resume"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bulk_resume_resp.status(), reqwest::StatusCode::OK);
    let resume_json: serde_json::Value = bulk_resume_resp.json().await.unwrap();
    assert_eq!(resume_json["total"], 1);

    // h) Partial failure test: one valid ID and one non-existent ID
    let fake_id = uuid::Uuid::new_v4().to_string();
    let partial_resp = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "ids": [download_id.clone(), fake_id.clone()],
            "action": "pause"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(partial_resp.status(), reqwest::StatusCode::OK);
    let partial_json: serde_json::Value = partial_resp.json().await.unwrap();
    assert_eq!(partial_json["total"], 2);
    assert!(partial_json["failed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == fake_id && f["code"] == "not_found"));

    // i) Bulk delete by ID
    let bulk_del_resp = client
        .post(format!("{}/api/v1/downloads/bulk-action", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "ids": [download_id.clone()],
            "action": "delete",
            "delete_file": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bulk_del_resp.status(), reqwest::StatusCode::OK);
    let del_json: serde_json::Value = bulk_del_resp.json().await.unwrap();
    assert_eq!(del_json["total"], 1);
    assert_eq!(del_json["succeeded"][0], download_id);

    // Verify post-deletion state: item is absent from GET /api/v1/downloads
    let list_resp = client
        .get(format!("{}/api/v1/downloads", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    let list_json: serde_json::Value = list_resp.json().await.unwrap();
    let items = list_json["items"].as_array().unwrap();
    assert!(
        !items.iter().any(|i| i["id"] == download_id),
        "Deleted item must not exist in list"
    );
}
