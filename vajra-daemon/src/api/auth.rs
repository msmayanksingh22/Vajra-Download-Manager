//! Authentication and CORS origin verification for the Vajra daemon.

use std::{path::Path, sync::Arc};

use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    Json,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::AppState;

/// Returns true if a candidate token string is a valid, usable API token.
/// Rejects placeholders (e.g. `"********"` or strings of all asterisks), empty strings,
/// or tokens shorter than 16 characters.
pub fn is_valid_token(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() || t.len() < 16 {
        return false;
    }
    if t == "********" || t.chars().all(|c| c == '*') {
        return false;
    }
    true
}

/// Establishes deterministic precedence between config.json api_token and api.token:
/// - If config.json contains an explicit non-empty api_token:
///   - use that token
///   - ensure api.token contains the same token (reconciling if different)
/// - Else if api.token file exists on disk with a non-empty token:
///   - use that token in memory (do not write back to config.json)
/// - Else:
///   - generate a cryptographically secure random 32-byte hex token
///   - write it to api.token with restrictive owner-only permissions
///   - keep the token in daemon memory (do NOT write to config.json)
pub fn bootstrap_api_token(config_token: &mut Option<String>) -> anyhow::Result<String> {
    let token_file_path = vajra_protocol::token_path();
    bootstrap_api_token_at(config_token, &token_file_path)
}

pub fn bootstrap_api_token_at(
    config_token: &mut Option<String>,
    token_file_path: &Path,
) -> anyhow::Result<String> {
    let file_token = if token_file_path.exists() {
        std::fs::read_to_string(token_file_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| is_valid_token(s))
    } else {
        None
    };

    let explicit_config = config_token
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| is_valid_token(s));

    let final_token = match (explicit_config, file_token) {
        (Some(cfg_tok), Some(file_tok)) => {
            if cfg_tok != file_tok {
                tracing::warn!(
                    "API token mismatch: config.json explicit token supersedes api.token file. Updating api.token."
                );
                write_restricted_token_file(token_file_path, &cfg_tok)?;
            }
            cfg_tok
        }
        (Some(cfg_tok), None) => {
            tracing::info!("Writing explicit config.json API token to api.token file.");
            write_restricted_token_file(token_file_path, &cfg_tok)?;
            cfg_tok
        }
        (None, Some(file_tok)) => {
            tracing::info!("Reusing existing daemon API token from api.token file.");
            file_tok
        }
        (None, None) => {
            tracing::info!("No API token found. Generating a secure random API token...");
            let mut bytes = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
            let gen_tok = hex::encode(bytes);
            write_restricted_token_file(token_file_path, &gen_tok)?;
            gen_tok
        }
    };

    *config_token = Some(final_token.clone());
    Ok(final_token)
}

pub fn write_restricted_token_file(path: &Path, token: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, token)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }
    }

    Ok(())
}

/// Constant-time verification of provided token against expected token.
/// Hashes both with SHA-256 before comparing to prevent length leaks and timing attacks.
pub fn verify_token_constant_time(provided: &str, expected: &str) -> bool {
    let prov_hash = Sha256::digest(provided.trim().as_bytes());
    let exp_hash = Sha256::digest(expected.trim().as_bytes());
    prov_hash.ct_eq(&exp_hash).into()
}

/// Validates whether a CORS Origin header value is an allowed local origin.
/// Must NOT use starts_with or substring checks.
pub fn is_allowed_origin(origin_str: &str, allowed_extension_ids: &[String]) -> bool {
    let Ok(parsed) = url::Url::parse(origin_str) else {
        return false;
    };

    // Reject origins with credentials, query strings, or fragments
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return false;
    }
    // Origin path must be empty or "/"
    if parsed.path() != "/" && !parsed.path().is_empty() {
        return false;
    }

    match parsed.scheme() {
        "http" | "https" => matches!(
            parsed.host_str(),
            Some("localhost") | Some("127.0.0.1") | Some("tauri.localhost")
        ),
        "tauri" => parsed.host_str() == Some("localhost"),
        "chrome-extension" | "moz-extension" => {
            if let Some(ext_id) = parsed.host_str() {
                allowed_extension_ids
                    .iter()
                    .any(|allowed| allowed == ext_id)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Authentication middleware for daemon routes.
pub async fn auth_middleware(
    state: Arc<AppState>,
    req: Request,
    next: middleware::Next,
) -> Response {
    let path = req.uri().path();

    // Skip auth for public endpoints
    if path == "/health"
        || path == "/setup"
        || path.starts_with("/swagger-ui")
        || path.starts_with("/api-docs")
    {
        return next.run(req).await;
    }

    let expected_token = state.config.read().await.api_token.clone();

    // If no token is configured (should not happen with bootstrap, but handled safely)
    let Some(expected) = expected_token else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "code": "unauthorized",
                    "message": "Authentication is enabled but no API token is configured"
                }
            })),
        )
            .into_response();
    };

    // 1. Try Authorization header: Bearer <token>
    let mut provided_token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| {
            if let Some(tok) = h.strip_prefix("Bearer ") {
                Some(tok)
            } else if let Some(tok) = h.strip_prefix("bearer ") {
                Some(tok)
            } else {
                None
            }
        })
        .map(str::trim);

    // 2. For streaming endpoints (SSE / WS / Spider), allow ?token=<token>
    if provided_token.is_none() {
        let is_streaming = path == "/api/v1/events"
            || path == "/events"
            || path == "/api/v1/ws"
            || path == "/ws"
            || path == "/api/v1/spider"
            || path == "/spider"
            || ((path.starts_with("/api/v1/downloads/") || path.starts_with("/downloads/"))
                && path.ends_with("/events"));
        if is_streaming {
            if let Some(query) = req.uri().query() {
                for pair in query.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        if k == "token" {
                            provided_token = Some(v);
                            break;
                        }
                    }
                }
            }
        }
    }

    if let Some(tok) = provided_token {
        if verify_token_constant_time(tok, &expected) {
            return next.run(req).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": {
                "code": "unauthorized",
                "message": "Missing or invalid authorization token"
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_origin() {
        let allowed_exts = vec!["abcdefghijklmnopabcdefghijklmnop".to_string()];

        // Allowed localhost origins
        assert!(is_allowed_origin("http://localhost", &allowed_exts));
        assert!(is_allowed_origin("http://localhost:3000", &allowed_exts));
        assert!(is_allowed_origin("http://localhost:6277", &allowed_exts));
        assert!(is_allowed_origin("http://127.0.0.1", &allowed_exts));
        assert!(is_allowed_origin("http://127.0.0.1:8080", &allowed_exts));
        assert!(is_allowed_origin("https://tauri.localhost", &allowed_exts));
        assert!(is_allowed_origin("tauri://localhost", &allowed_exts));

        // Allowed extension
        assert!(is_allowed_origin(
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop",
            &allowed_exts
        ));
        assert!(is_allowed_origin(
            "moz-extension://abcdefghijklmnopabcdefghijklmnop",
            &allowed_exts
        ));

        // Forbidden lookalikes
        assert!(!is_allowed_origin(
            "http://localhost.evil.com",
            &allowed_exts
        ));
        assert!(!is_allowed_origin(
            "http://localhost:3000.evil.org",
            &allowed_exts
        ));
        assert!(!is_allowed_origin(
            "http://127.0.0.1.attacker.com",
            &allowed_exts
        ));
        assert!(!is_allowed_origin(
            "http://evil.com/localhost",
            &allowed_exts
        ));
        assert!(!is_allowed_origin("http://attacker.com", &allowed_exts));

        // Unknown extensions
        assert!(!is_allowed_origin(
            "chrome-extension://unknownextensionid",
            &allowed_exts
        ));
        assert!(!is_allowed_origin("moz-extension://otherid", &allowed_exts));

        // Malformed
        assert!(!is_allowed_origin("not-a-url", &allowed_exts));
        assert!(!is_allowed_origin(
            "http://user:pass@localhost",
            &allowed_exts
        ));
    }

    #[test]
    fn test_verify_token_constant_time() {
        assert!(verify_token_constant_time(
            "my-secret-token",
            "my-secret-token"
        ));
        assert!(!verify_token_constant_time(
            "my-secret-token",
            "wrong-token"
        ));
        assert!(!verify_token_constant_time("short", "longer-token-here"));
    }

    #[test]
    fn test_bootstrap_api_token_precedence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let token_path = temp_dir.path().join("api.token");

        // Case 1: Both config and file exist but disagree -> config wins, file updated
        std::fs::write(&token_path, "file-token-12345678").unwrap();
        let mut cfg_token = Some("config-token-45678901".to_string());
        let final_token = bootstrap_api_token_at(&mut cfg_token, &token_path).unwrap();
        assert_eq!(final_token, "config-token-45678901");
        assert_eq!(cfg_token, Some("config-token-45678901".to_string()));
        let on_disk = std::fs::read_to_string(&token_path).unwrap();
        assert_eq!(on_disk, "config-token-45678901");

        // Case 2: Config token exists, file does not exist -> writes config token to file
        let _ = std::fs::remove_file(&token_path);
        let mut cfg_token = Some("config-token-only-1234".to_string());
        let final_token = bootstrap_api_token_at(&mut cfg_token, &token_path).unwrap();
        assert_eq!(final_token, "config-token-only-1234");
        let on_disk = std::fs::read_to_string(&token_path).unwrap();
        assert_eq!(on_disk, "config-token-only-1234");

        // Case 3: Config is None, file exists -> file token used, kept in memory, file untouched
        let mut cfg_token = None;
        let final_token = bootstrap_api_token_at(&mut cfg_token, &token_path).unwrap();
        assert_eq!(final_token, "config-token-only-1234");
        assert_eq!(cfg_token, Some("config-token-only-1234".to_string()));

        // Case 4: Both None -> generates 64-char hex token, writes to file, sets in memory
        let _ = std::fs::remove_file(&token_path);
        let mut cfg_token = None;
        let final_token = bootstrap_api_token_at(&mut cfg_token, &token_path).unwrap();
        assert_eq!(final_token.len(), 64);
        assert_eq!(cfg_token, Some(final_token.clone()));
        let on_disk = std::fs::read_to_string(&token_path).unwrap();
        assert_eq!(on_disk, final_token);
    }

    #[test]
    fn test_bootstrap_api_token_placeholder() {
        let temp_dir = tempfile::tempdir().unwrap();
        let token_path = temp_dir.path().join("api.token");

        // Test is_valid_token helper
        assert!(!is_valid_token(""));
        assert!(!is_valid_token("   "));
        assert!(!is_valid_token("********"));
        assert!(!is_valid_token("****************")); // 16 asterisks
        assert!(!is_valid_token("short"));
        assert!(is_valid_token("a-valid-secure-token-12345"));

        // If config contains placeholder "********", it must NOT become the token
        let mut cfg_token = Some("********".to_string());
        let final_token = bootstrap_api_token_at(&mut cfg_token, &token_path).unwrap();
        assert_ne!(final_token, "********");
        assert_eq!(final_token.len(), 64);
        assert_eq!(cfg_token, Some(final_token.clone()));

        // If api.token on disk contains placeholder "********", it is ignored
        std::fs::write(&token_path, "********").unwrap();
        let mut cfg_token2 = None;
        let final_token2 = bootstrap_api_token_at(&mut cfg_token2, &token_path).unwrap();
        assert_ne!(final_token2, "********");
        assert_eq!(final_token2.len(), 64);
    }
}
